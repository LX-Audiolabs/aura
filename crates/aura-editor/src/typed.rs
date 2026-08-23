//! Typed editor layer: `LxPluginContext<P>` + `LxSlintEditor<P, C>` with UI zoom.
//!
//! Builds on the untyped [`AuraSlintEditor`](super::AuraSlintEditor) foundation.
//! Plugin authors use this for typed param access, `automate()`, and product zoom.

use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use aura_baseview::SlintParentedWindow;
use aura_baseview::slint_window::SlintWindow;
use aura_baseview::{RequestResizeFn, SizePolicy, pack_size, to_physical_px};
use aura_core::editor::{Editor, PluginContext, RawWindowHandle};
use aura_params::Params;
use baseview::{WindowSettings, gl::GlConfig};
use slint::ComponentHandle;

use super::ui_zoom::UiZoom;
use super::{map_baseview_handle, parent};

/// Typed product context: Arc params + host bridge helpers.
pub struct LxPluginContext<P: Params + 'static> {
    pub params: Arc<P>,
    pub host: PluginContext,
}

impl<P: Params + 'static> Clone for LxPluginContext<P> {
    fn clone(&self) -> Self {
        Self {
            params: Arc::clone(&self.params),
            host: self.host.clone(),
        }
    }
}

impl<P: Params + 'static> LxPluginContext<P> {
    #[must_use]
    pub fn new(params: Arc<P>, host: PluginContext) -> Self {
        Self { params, host }
    }

    /// Begin + set + end (or plain set when no bridge).
    pub fn automate(&self, id: impl Into<u32>, normalized: f64) {
        let id = id.into();
        if let Some(bridge) = &self.host.bridge {
            bridge.begin_edit(id);
            bridge.set_param(id, normalized);
            bridge.end_edit(id);
        } else {
            self.params.set_normalized(id, normalized);
        }
    }

    #[must_use]
    pub fn get_param(&self, id: impl Into<u32>) -> f64 {
        let id = id.into();
        if let Some(bridge) = &self.host.bridge {
            bridge.get_param(id)
        } else {
            self.params.get_normalized(id).unwrap_or(0.0)
        }
    }

    #[must_use]
    pub fn get_param_plain(&self, id: impl Into<u32>) -> f64 {
        let id = id.into();
        if let Some(bridge) = &self.host.bridge {
            bridge.get_param_plain(id)
        } else {
            self.params.get_plain(id).unwrap_or(0.0)
        }
    }

    #[must_use]
    pub fn format_param(&self, id: impl Into<u32>) -> String {
        let id = id.into();
        let plain = self.get_param_plain(id);
        self.params
            .format_value(id, plain)
            .unwrap_or_else(|| format!("{plain:.2}"))
    }

    #[must_use]
    pub fn request_resize(&self, w: u32, h: u32) -> bool {
        self.host
            .bridge
            .as_ref()
            .is_some_and(|b| b.request_resize(w, h))
    }
}

impl<P: Params + 'static> Deref for LxPluginContext<P> {
    type Target = P;
    fn deref(&self) -> &P {
        &self.params
    }
}

/// f32 reads for dirty sync macros.
pub trait PluginContextReadF32 {
    fn get_param(&self, id: impl Into<u32>) -> f32;
}

impl<P: Params + 'static> PluginContextReadF32 for LxPluginContext<P> {
    fn get_param(&self, id: impl Into<u32>) -> f32 {
        #[allow(clippy::cast_possible_truncation)]
        {
            LxPluginContext::get_param(self, id) as f32
        }
    }
}

/// Discrete index 0..count-1 → normalized 0..1.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn discrete_norm(index: usize, count: usize) -> f64 {
    if count <= 1 {
        0.0
    } else {
        (index.min(count - 1) as f64) / ((count - 1) as f64)
    }
}

/// Normalized 0..1 → discrete index.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
pub fn discrete_index(norm: f64, count: usize) -> usize {
    if count <= 1 {
        0
    } else {
        let n = norm.clamp(0.0, 1.0);
        ((n * (count - 1) as f64).round() as usize).min(count - 1)
    }
}

pub type BuildFn<P, C> = Arc<dyn Fn(LxPluginContext<P>) -> C + Send + Sync>;
pub type SyncFn<P, C> = Arc<dyn Fn(&C, &LxPluginContext<P>) + Send + Sync>;

/// Slint editor with product UI zoom.
pub struct LxSlintEditor<P, C>
where
    P: Params + 'static,
    C: ComponentHandle + 'static,
{
    params: Arc<P>,
    design_size: (u32, u32),
    build: BuildFn<P, C>,
    sync: SyncFn<P, C>,
    window: Option<SlintParentedWindow>,
    can_resize: bool,
    min_size: (u32, u32),
    max_size: (u32, u32),
    ui_zoom: UiZoom,
    pending_size: Arc<AtomicU64>,
    native_handle: Option<RawWindowHandle>,
    /// True once the host called `set_scale`. When it never does (Bitwig on
    /// Win32), the editor adopts the real OS DPI itself on open.
    host_scale_provided: Arc<AtomicBool>,
}

// SAFETY: baseview Window is GUI-thread only (host contract).
unsafe impl<P, C> Send for LxSlintEditor<P, C>
where
    P: Params + 'static,
    C: ComponentHandle + 'static,
{
}

impl<P, C> LxSlintEditor<P, C>
where
    P: Params + 'static,
    C: ComponentHandle + 'static,
{
    pub fn new(
        params: Arc<P>,
        size: (u32, u32),
        build: impl Fn(LxPluginContext<P>) -> C + Send + Sync + 'static,
        sync: impl Fn(&C, &LxPluginContext<P>) + Send + Sync + 'static,
    ) -> Self {
        Self::new_with_zoom(params, UiZoom::new(size.0, size.1), build, sync)
    }

    pub fn new_with_zoom(
        params: Arc<P>,
        ui_zoom: UiZoom,
        build: impl Fn(LxPluginContext<P>) -> C + Send + Sync + 'static,
        sync: impl Fn(&C, &LxPluginContext<P>) + Send + Sync + 'static,
    ) -> Self {
        let design_size = ui_zoom.design_size();
        let zoomed = ui_zoom.zoomed_size();
        Self {
            params,
            design_size,
            build: Arc::new(build),
            sync: Arc::new(sync),
            window: None,
            can_resize: false,
            min_size: zoomed,
            max_size: zoomed,
            ui_zoom,
            pending_size: Arc::new(AtomicU64::new(0)),
            native_handle: None,
            host_scale_provided: Arc::new(AtomicBool::new(false)),
        }
    }

    #[must_use]
    pub fn ui_zoom(&self) -> UiZoom {
        self.ui_zoom.clone()
    }

    #[must_use]
    pub fn resizable(mut self, value: bool) -> Self {
        self.can_resize = value;
        let z = self.ui_zoom.zoomed_size();
        self.min_size = z;
        self.max_size = z;
        self
    }

    fn size_policy(&self) -> SizePolicy {
        let zoomed = self.ui_zoom.zoomed_size();
        SizePolicy {
            design_size: self.design_size,
            min_size: zoomed,
            max_size: zoomed,
            can_resize: self.can_resize,
            scale: self.ui_zoom.scale(),
            host_driven_scale: true,
            pending_size: Arc::clone(&self.pending_size),
        }
    }

    fn sync_minmax_to_zoom(&mut self) {
        let z = self.ui_zoom.zoomed_size();
        self.min_size = z;
        self.max_size = z;
    }
}

impl<P, C> Editor for LxSlintEditor<P, C>
where
    P: Params + 'static,
    C: ComponentHandle + 'static,
{
    fn size(&self) -> (u32, u32) {
        self.ui_zoom.zoomed_size()
    }

    fn open(&mut self, parent: RawWindowHandle, context: PluginContext) {
        self.close();
        self.pending_size.store(0, Ordering::Relaxed);
        self.sync_minmax_to_zoom();

        let lx = LxPluginContext::new(self.params.clone(), context.clone());
        let parent_window = parent::ParentedWindow::from_raw(parent);

        let (dw, dh) = self.design_size;
        let content_scale = self.ui_zoom.scale().get();
        let phys_w = to_physical_px(dw, content_scale);
        let phys_h = to_physical_px(dh, content_scale);

        let options = WindowSettings::new()
            .with_title("LX Audiolabs")
            .with_size(baseview::dpi::PhysicalSize::new(phys_w, phys_h))
            .with_gl_config(GlConfig {
                alpha_bits: 8,
                ..GlConfig::default()
            });

        let build = Arc::clone(&self.build);
        let sync = Arc::clone(&self.sync);
        let policy = self.size_policy();
        let params = self.params.clone();

        let request_resize: Option<RequestResizeFn> = {
            let zoom = self.ui_zoom.clone();
            context.bridge.clone().map(|bridge| {
                Arc::new(move |_rw: u32, _rh: u32| {
                    let (w, h) = zoom.zoomed_size();
                    bridge.request_resize(w, h)
                }) as RequestResizeFn
            })
        };

        let window = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            SlintWindow::open_parented_with_policy(
                &parent_window,
                options,
                lx,
                move |state: &mut LxPluginContext<P>| build(state.clone()),
                move |component: &C, state: &mut LxPluginContext<P>| {
                    let _ = &params;
                    sync(component, state);
                },
                policy,
                request_resize,
            )
        }));

        match window {
            Ok(Ok(w)) => {
                let (zw, zh) = self.ui_zoom.zoomed_size();
                if let Some(b) = &context.bridge {
                    let _ = b.request_resize(zw, zh);
                }
                let _ = w.resize(baseview::dpi::Size::Physical(
                    baseview::dpi::PhysicalSize::new(phys_w, phys_h),
                ));

                // Windows: hosts that never call `set_scale` (Bitwig on Win32)
                // leave content scale at 1.0 while the OS gives the per-monitor
                // child a HiDPI backbuffer — the FemtoVG viewport (logical ×
                // content_scale) then under-fills it, so the UI renders into a
                // corner with grey margins. baseview already knows the real DPI
                // of the parented child, so adopt it: render scale becomes
                // `os_dpi × ui_zoom`, and the hint feeds `get_size` so the host
                // frame matches. Layout stays fixed (design logical) — Slint
                // scales uniformly, nothing reflows. No-op at 100% (os == 1.0)
                // and skipped when the host does drive scale.
                #[cfg(target_os = "windows")]
                if !self.host_scale_provided.load(Ordering::Relaxed) {
                    let os_dpi = w.size().scale_factor;
                    if os_dpi.is_finite() && os_dpi > 1.0 {
                        self.ui_zoom.set_host_scale(os_dpi);
                        if let Some(b) = &context.bridge {
                            b.set_scale_hint(os_dpi);
                            let _ = b.request_resize(zw, zh);
                        }
                    }
                }

                self.native_handle = map_baseview_handle(w.raw_handle());
                self.window = Some(w);
            }
            Ok(Err(e)) => {
                log::error!(
                    "LX UI: OpenGL/FemtoVG init failed — editor left closed (audio still runs). Detail: {e}"
                );
            }
            Err(_) => {
                log::error!(
                    "LX UI: window open panicked — editor left closed. Need OpenGL 3.2 Core. Audio still runs."
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
        self.can_resize && self.min_size != self.max_size
    }

    fn min_size(&self) -> (u32, u32) {
        self.min_size
    }

    fn max_size(&self) -> (u32, u32) {
        self.max_size
    }

    fn set_scale(&mut self, factor: f64) {
        if factor.is_finite() && factor > 0.0 {
            // Host drives scale — mark it so open() doesn't also adopt OS DPI.
            self.host_scale_provided.store(true, Ordering::Relaxed);
            self.ui_zoom.set_host_scale(factor);
        }
    }

    fn set_size(&mut self, width: u32, height: u32) -> bool {
        if width == 0 || height == 0 {
            return false;
        }
        let zoomed = self.ui_zoom.zoomed_size();
        if (width, height) != zoomed {
            self.pending_size
                .store(pack_size(self.design_size), Ordering::Release);
            return false;
        }
        true
    }

    fn idle(&mut self) {
        if let Some(window) = &mut self.window {
            window.host_main_thread_callback();
        }
    }

    fn native_handle(&self) -> Option<RawWindowHandle> {
        self.native_handle
    }
}

impl<P, C> From<LxSlintEditor<P, C>> for Box<dyn Editor>
where
    P: Params + 'static,
    C: ComponentHandle + 'static,
{
    fn from(editor: LxSlintEditor<P, C>) -> Self {
        Box::new(editor)
    }
}
