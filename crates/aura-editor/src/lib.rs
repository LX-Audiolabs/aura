//! AURA host-facing **Editor** layer.
//!
//! [`AuraSlintEditor`] adapts a Slint component (living on the
//! [`aura_baseview`] window stack) to [`aura_core::editor::Editor`], so format
//! wrappers (CLAP GUI, later VST3/LV2) can parent it into a host window.
//!
//! Window/platform/renderers stay in **`aura-baseview`** (re-exported here);
//! only the host adapter couples to `aura-core`.
//!
//! # Example
//!
//! ```ignore
//! use aura_editor::AuraSlintEditor;
//!
//! fn editor(_params: Arc<MyParams>) -> Option<Box<dyn Editor>> {
//!     Some(
//!         AuraSlintEditor::new(
//!             (800, 600),
//!             |ctx| {
//!                 let ui = MyPluginUi::new().unwrap();
//!                 // wire ui callbacks → ctx.bridge / ctx.params …
//!                 ui
//!             },
//!             |ui, ctx| {
//!                 // per-frame: push host param values into the component
//!             },
//!         )
//!         .into_editor(),
//!     )
//! }
//! ```

mod parent;
pub mod typed;
pub mod ui_zoom;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use baseview::WindowSettings;
use slint::ComponentHandle;

use aura_core::editor::{Editor, PluginContext, RawWindowHandle};

pub(crate) fn map_baseview_handle(
    raw: Option<raw_window_handle::RawWindowHandle>,
) -> Option<RawWindowHandle> {
    use raw_window_handle::RawWindowHandle as Rwh;
    raw.and_then(|h| match h {
        #[cfg(target_os = "windows")]
        Rwh::Win32(h) => Some(RawWindowHandle::Win32(h.hwnd.get() as *mut std::ffi::c_void)),
        #[cfg(target_os = "macos")]
        Rwh::AppKit(h) => Some(RawWindowHandle::AppKit(h.ns_view.as_ptr())),
        #[cfg(target_os = "linux")]
        Rwh::Xlib(h) => Some(RawWindowHandle::X11(h.window as u64)),
        _ => None,
    })
}

// Window stack (Slint + baseview + renderer backends) stays reachable
// through this crate so plugin authors need one UI dependency. Also brings
// `SlintWindow`, `SizePolicy`, `EditorScale`, … into local scope.
#[allow(unused_imports)]
pub use aura_baseview::*;

/// Build closure: creates the Slint component and wires UI callbacks.
/// Called once when the host opens the editor.
pub type BuildFn<C> = Arc<dyn Fn(PluginContext) -> C + Send + Sync>;

/// Per-frame sync closure: pushes host parameter values into the component.
pub type SyncFn<C> = Arc<dyn Fn(&C, &PluginContext) + Send + Sync>;

/// Slint-based editor implementing [`aura_core::editor::Editor`].
///
/// Generic over the concrete Slint component type, because the GPU-backed
/// [`SlintWindow`] needs to own the generated component.
pub struct AuraSlintEditor<C>
where
    C: ComponentHandle + 'static,
{
    /// Design logical size (layout coordinates — host frame in v1).
    design_size: (u32, u32),
    build: BuildFn<C>,
    sync: SyncFn<C>,
    window: Option<SlintParentedWindow>,
    can_resize: bool,
    min_size: (u32, u32),
    max_size: (u32, u32),
    /// Shared content-scale cell (host `set_scale` ↔ window handler).
    scale: EditorScale,
    /// Packed design logical size for handler reconcile after host `set_size`.
    pending_size: Arc<AtomicU64>,
    /// Native widget handle of the opened child window, if available.
    native_handle: Option<RawWindowHandle>,
}

// SAFETY: baseview::Window holds raw native window pointers (HWND/NSView)
// and is not auto-Send. Hosts call Editor::open/close from a single GUI
// thread, never concurrently. Same pattern as truce-slint.
unsafe impl<C> Send for AuraSlintEditor<C> where C: ComponentHandle + 'static {}

impl<C> AuraSlintEditor<C>
where
    C: ComponentHandle + 'static,
{
    /// Create a new editor adapter.
    ///
    /// - `size` is the fixed design size in logical pixels.
    /// - `build` is called once when the host opens the editor. It receives a
    ///   [`PluginContext`] (params + optional host bridge) and must return
    ///   the concrete Slint component.
    /// - `sync` is called every frame to copy host parameter values into the
    ///   component.
    pub fn new(
        size: (u32, u32),
        build: impl Fn(PluginContext) -> C + Send + Sync + 'static,
        sync: impl Fn(&C, &PluginContext) + Send + Sync + 'static,
    ) -> Self {
        Self {
            design_size: size,
            build: Arc::new(build),
            sync: Arc::new(sync),
            window: None,
            can_resize: false,
            min_size: size,
            max_size: size,
            scale: EditorScale::new(1.0),
            pending_size: Arc::new(AtomicU64::new(0)),
            native_handle: None,
        }
    }

    /// Report resizability to the host. Set real bounds with
    /// [`Self::with_min_size`] / [`Self::with_max_size`].
    #[must_use]
    pub fn resizable(mut self, value: bool) -> Self {
        self.can_resize = value;
        self
    }

    #[must_use]
    pub fn with_min_size(mut self, min: (u32, u32)) -> Self {
        self.min_size = min;
        self
    }

    #[must_use]
    pub fn with_max_size(mut self, max: (u32, u32)) -> Self {
        self.max_size = max;
        self
    }

    fn size_policy(&self) -> SizePolicy {
        // Plugins: content scale is host `set_scale` only (default 1.0),
        // never raw OS DPI — CLAP/VST3 report HWND size as size() × host
        // scale, so the child must match that, not GetDpiForWindow/Xft.dpi.
        SizePolicy {
            design_size: self.design_size,
            min_size: self.min_size,
            max_size: self.max_size,
            can_resize: self.can_resize,
            scale: self.scale.clone(),
            host_driven_scale: true,
            pending_size: Arc::clone(&self.pending_size),
        }
    }
}

impl<C> Editor for AuraSlintEditor<C>
where
    C: ComponentHandle + 'static,
{
    fn size(&self) -> (u32, u32) {
        self.design_size
    }

    fn open(&mut self, parent: RawWindowHandle, context: PluginContext) {
        // Drop previous window if host re-opens without close.
        self.close();

        // Reset stale set_size / native handle from a previous open.
        self.pending_size.store(0, Ordering::Relaxed);
        self.native_handle = None;

        let parent_window = parent::ParentedWindow::from_raw(parent);

        // Physical = design × host scale.
        let (dw, dh) = self.design_size;
        let content_scale = self.scale.get();
        let phys_w = to_physical_px(dw, content_scale);
        let phys_h = to_physical_px(dh, content_scale);

        // FemtoVG needs OpenGL (baseview opengl feature); other backends
        // ignore the GL config. alpha_bits=8 helps embedded plugin windows
        // in DAW hosts.
        let options = WindowSettings::new()
            .with_title("AURA")
            .with_size(baseview::dpi::PhysicalSize::new(phys_w, phys_h))
            .with_gl_config(baseview::gl::GlConfig {
                alpha_bits: 8,
                ..baseview::gl::GlConfig::default()
            });

        let build = Arc::clone(&self.build);
        let sync = Arc::clone(&self.sync);
        let policy = self.size_policy();

        // Host resize requests go through the bridge (CLAP gui_request_resize).
        let request_resize: Option<RequestResizeFn> = context.bridge.clone().map(|bridge| {
            Arc::new(move |w: u32, h: u32| bridge.request_resize(w, h)) as RequestResizeFn
        });

        let window = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            slint_window::SlintWindow::open_parented_with_policy(
                &parent_window,
                options,
                context,
                move |state: &mut PluginContext| build(state.clone()),
                move |component: &C, state: &mut PluginContext| sync(component, state),
                policy,
                request_resize,
            )
        }));

        match window {
            Ok(Ok(w)) => {
                let _ = w.resize(baseview::dpi::Size::Physical(
                    baseview::dpi::PhysicalSize::new(phys_w, phys_h),
                ));
                self.native_handle = map_baseview_handle(w.raw_handle());
                self.window = Some(w);
            }
            // Soft-fail: editor stays closed; DSP continues. Typical on old
            // Linux/mac without OpenGL 3.2 Core / broken GLX embeds.
            Ok(Err(e)) => {
                log::error!(
                    "AURA UI: window open failed ({e}) — editor left closed, audio still runs"
                );
            }
            Err(_) => {
                log::error!(
                    "AURA UI: window open panicked — editor left closed, audio still runs. \
                     Need OpenGL 3.2 Core (or newer) for the FemtoVG backend."
                );
            }
        }
    }

    fn close(&mut self) {
        if let Some(window) = self.window.take() {
            window.close();
        }
        self.native_handle = None;
    }

    fn show(&mut self) {
        if let Some(window) = &self.window {
            let _ = window.show();
        }
    }

    fn hide(&mut self) {
        if let Some(window) = &self.window {
            let _ = window.hide();
        }
    }

    fn can_resize(&self) -> bool {
        self.can_resize
    }

    fn min_size(&self) -> (u32, u32) {
        self.min_size
    }

    fn max_size(&self) -> (u32, u32) {
        self.max_size
    }

    fn set_scale(&mut self, scale: f64) {
        self.scale.set(scale);
    }

    fn idle(&mut self) {
        // X11 needs main-thread callbacks pumped; Windows/macOS are no-ops.
        if let Some(window) = &mut self.window {
            window.host_main_thread_callback();
        }
    }

    fn native_handle(&self) -> Option<RawWindowHandle> {
        self.native_handle
    }

    fn set_size(&mut self, width: u32, height: u32) -> bool {
        if width == 0 || height == 0 {
            return false;
        }
        if self.can_resize {
            let (w, h) = aura_baseview::fit_size(width, height, self.min_size, self.max_size);
            self.design_size = (w, h);
            self.pending_size
                .store(pack_size((w, h)), Ordering::Release);
            return true;
        }
        // Fixed layout: host set_size from multi-monitor / pane chrome must
        // NOT reflow the scene. Re-assert design logical for Slint.
        if (width, height) != self.design_size {
            self.pending_size
                .store(pack_size(self.design_size), Ordering::Release);
            return false;
        }
        true
    }
}

impl<C> From<AuraSlintEditor<C>> for Box<dyn Editor>
where
    C: ComponentHandle + 'static,
{
    fn from(editor: AuraSlintEditor<C>) -> Self {
        Box::new(editor)
    }
}
