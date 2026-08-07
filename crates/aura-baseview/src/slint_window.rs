use crate::translate::translate_mouse_button;
#[cfg(feature = "backend-femtovg")]
use crate::baseview_slint_window_adapter::BaseviewSlintWindowAdapter;
#[cfg(feature = "backend-skia")]
use crate::skia_window_adapter::SkiaWindowAdapter;
use crate::platform;
use crate::scale::{
    to_physical_px, unpack_size, EditorScale, RequestResizeFn, SizePolicy,
};
#[cfg(feature = "backend-wgpu")]
use crate::blit::BlitPipeline;
#[cfg(feature = "backend-wgpu")]
use crate::software_renderer::render_to_rgba;
use baseview::{
    dpi, Event, EventStatus, HandlerError, Window, WindowContext, WindowSettings, WindowSize,
};
use keyboard_types::{Code, Key};
use raw_window_handle::HasWindowHandle;
#[cfg(feature = "backend-wgpu")]
use raw_window_handle::HasDisplayHandle;
use slint::{platform::WindowEvent, ComponentHandle, LogicalPosition, LogicalSize, PhysicalSize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[cfg(feature = "backend-wgpu")]
use slint::platform::software_renderer::{MinimalSoftwareWindow, PremultipliedRgbaColor};

#[derive(Default, Debug, Clone, PartialEq)]
pub enum KeyCapture {
    #[default]
    CaptureAll,
    IgnoreAll,
    CaptureKeys(Vec<Key>),
    IgnoreKeys(Vec<Key>),
}

/// Wgpu rendering state, created once when the window opens.
#[cfg(feature = "backend-wgpu")]
pub(crate) struct WgpuState {
    slint_window: Rc<MinimalSoftwareWindow>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: RefCell<wgpu::SurfaceConfiguration>,
    blit: RefCell<BlitPipeline>,
    /// Scratch buffers reused every frame.
    px_buf: RefCell<Vec<PremultipliedRgbaColor>>,
    rgba_buf: RefCell<Vec<u8>>,
}

pub struct SlintWindow<C, S, U>
where
    C: ComponentHandle + 'static,
    S: Send + 'static,
    U: Fn(&C, &mut S) + Send + 'static,
{
    pub component: C,
    pub state: RefCell<S>,
    pub update: U,
    #[cfg(all(feature = "backend-femtovg", not(feature = "backend-skia")))]
    pub adapter: Rc<BaseviewSlintWindowAdapter>,
    #[cfg(all(feature = "backend-skia", not(feature = "backend-femtovg")))]
    pub adapter: Rc<SkiaWindowAdapter>,
    #[cfg(all(
        feature = "backend-wgpu",
        not(feature = "backend-femtovg"),
        not(feature = "backend-skia")
    ))]
    pub(crate) wgpu: WgpuState,
    /// Kept for OS keyboard focus on click (Windows `WS_CHILD`).
    window_ctx: WindowContext,
    pub key_capture: KeyCapture,
    pub last_cursor_pos: RefCell<LogicalPosition>,
    pub pending_clipboard_paste: Cell<bool>,
    /// Self-tracked Control state: some hosts deliver Ctrl+V without the
    /// Control modifier flag on the V event (and drop pure Control keys),
    /// which would otherwise type a literal "v" instead of pasting.
    pub ctrl_held: Cell<bool>,
    /// Content scale: host `set_scale` / explicit policy — **not** raw OS DPI.
    /// CLAP/VST3 report `size() * host_scale` as HWND pixels; the child must
    /// match that, not `size() * GetDpiForWindow` (that mismatch clips the UI
    /// and leaves gray host margins on multi-monitor).
    scale: EditorScale,
    last_applied_scale: Cell<f32>,
    /// When true, baseview OS scale from `Resized` must not overwrite `scale`.
    host_driven_scale: bool,
    /// Current logical size driving the Slint scene (design size; not host HWND).
    logical_size: Cell<(u32, u32)>,
    /// Packed `(w,h)` from `Editor::set_size` only (not host Resized).
    pending_size: Arc<AtomicU64>,
    /// Corrective host resize (logical), issued from `on_frame`.
    pending_host_correct: Cell<Option<(u32, u32)>>,
    /// Optional host `request_resize(logical_w, logical_h)`.
    request_resize: Option<RequestResizeFn>,
    /// Frame counter for throttled host re-assert (avoid resize spam).
    frames: Cell<u32>,
}

/// Field bundle built by `SlintWindow::init_policy_fields`.
type PolicyFields = (
    EditorScale,
    Cell<f32>,
    bool,
    Cell<(u32, u32)>,
    Arc<AtomicU64>,
    Cell<Option<(u32, u32)>>,
    Option<RequestResizeFn>,
    Cell<u32>,
);

impl<C, S, U> SlintWindow<C, S, U>
where
    C: ComponentHandle,
    S: Send + 'static,
    U: Fn(&C, &mut S) + Send + 'static,
{
    #[cfg(all(feature = "backend-femtovg", not(feature = "backend-skia"), not(feature = "backend-wgpu")))]
    fn slint_window_ref(&self) -> &slint::Window {
        &self.adapter.slint_window
    }
    #[cfg(all(feature = "backend-skia", not(feature = "backend-femtovg"), not(feature = "backend-wgpu")))]
    fn slint_window_ref(&self) -> &slint::Window {
        &self.adapter.slint_window
    }
    #[cfg(all(feature = "backend-wgpu", not(feature = "backend-femtovg"), not(feature = "backend-skia")))]
    fn slint_window_ref(&self) -> &slint::Window {
        &self.wgpu.slint_window
    }

    /// Desired child HWND size in **physical pixels** = logical × content scale.
    /// Must match what CLAP/VST3 report via `get_size` (logical × `host_scale`).
    fn desired_physical(lw: u32, lh: u32, scale: f64) -> (u32, u32) {
        (to_physical_px(lw, scale), to_physical_px(lh, scale))
    }

    /// Resize the baseview child to an exact physical pixel size.
    /// Using physical (not logical) avoids baseview re-multiplying by OS DPI.
    fn resize_child_physical(&self, phys_w: u32, phys_h: u32) {
        let _ = self
            .window_ctx
            .resize(dpi::PhysicalSize::new(phys_w.max(1), phys_h.max(1)));
    }

    /// Apply logical size + content scale to the Slint window / GPU surface.
    ///
    /// Order matches truce-slint: `ScaleFactorChanged` then `Resized`.
    /// Physical adapter size is always `logical × content_scale` and the
    /// child HWND is forced to the same physical extent.
    fn apply_geometry(&self, lw: u32, lh: u32, scale: f64) {
        let (phys_w, phys_h) = Self::desired_physical(lw, lh, scale);
        #[allow(clippy::cast_possible_truncation)]
        let scale_f32 = scale as f32;

        // Keep HWND pixel size in lockstep with the renderer (no OS DPI multiply).
        self.resize_child_physical(phys_w, phys_h);

        #[cfg(all(
            feature = "backend-femtovg",
            not(feature = "backend-skia"),
            not(feature = "backend-wgpu")
        ))]
        {
            self.adapter
                .update_size(PhysicalSize::new(phys_w, phys_h));
            self.adapter.slint_window.dispatch_event(
                slint::platform::WindowEvent::ScaleFactorChanged {
                    scale_factor: scale_f32,
                },
            );
            #[allow(clippy::cast_precision_loss)]
            self.adapter
                .slint_window
                .dispatch_event(slint::platform::WindowEvent::Resized {
                    size: LogicalSize::new(lw as f32, lh as f32),
                });
        }
        #[cfg(all(
            feature = "backend-skia",
            not(feature = "backend-femtovg"),
            not(feature = "backend-wgpu")
        ))]
        {
            self.adapter
                .update_size(PhysicalSize::new(phys_w, phys_h));
            self.adapter.slint_window.dispatch_event(
                slint::platform::WindowEvent::ScaleFactorChanged {
                    scale_factor: scale_f32,
                },
            );
            #[allow(clippy::cast_precision_loss)]
            self.adapter
                .slint_window
                .dispatch_event(slint::platform::WindowEvent::Resized {
                    size: LogicalSize::new(lw as f32, lh as f32),
                });
        }
        #[cfg(all(
            feature = "backend-wgpu",
            not(feature = "backend-femtovg"),
            not(feature = "backend-skia")
        ))]
        {
            self.wgpu.slint_window.dispatch_event(
                slint::platform::WindowEvent::ScaleFactorChanged {
                    scale_factor: scale_f32,
                },
            );
            self.wgpu.slint_window.dispatch_event(
                slint::platform::WindowEvent::Resized {
                    size: LogicalSize::new(lw as f32, lh as f32),
                },
            );
            let mut sc = self.wgpu.surface_config.borrow_mut();
            sc.width = phys_w.max(1);
            sc.height = phys_h.max(1);
            self.wgpu.surface.configure(&self.wgpu.device, &sc);
            drop(sc);
            self.wgpu
                .blit
                .borrow_mut()
                .resize(&self.wgpu.device, phys_w, phys_h);
        }

        self.logical_size.set((lw, lh));
        self.last_applied_scale.set(scale_f32);
    }

    /// Ask the host (and child) to match design logical size.
    fn queue_host_correct(&self, fitted: (u32, u32)) {
        self.pending_host_correct.set(Some(fitted));
    }

    /// Whether this frame may call host `request_resize` (embed push-back).
    ///
    /// Linux DAWs (esp. Bitwig) often ignore CLAP resize and *grow* the frame
    /// when we push back — a resize fight that clips the footer and makes
    /// knobs/sliders feel stuttery. truce-slint only host-corrects on macOS;
    /// we still correct once at open (`LxSlintEditor`) and re-assert the child.
    #[inline]
    fn host_resize_pushback_allowed() -> bool {
        // Windows: host push-back fixed multi-monitor DPI clips (Bitwig/Reaper).
        // macOS: needed for backing-scale handoff.
        // Linux: child-only re-assert — no host spam.
        !cfg!(target_os = "linux")
    }

    /// Keep child window + Slint geometry on the content scale.
    ///
    /// Critical: do **not** call `apply_geometry` every frame on a physical
    /// mismatch. That re-dispatches `ScaleFactorChanged` + Resized + GL surface
    /// resize every tick → knob/slider jank under Linux embeds where the host
    /// and child disagree for a while (or forever).
    fn reconcile_pending(&self) {
        let n = self.frames.get().wrapping_add(1);
        self.frames.set(n);

        // Editor::set_size may change design logical size (rare for LX).
        let pending = unpack_size(self.pending_size.swap(0, Ordering::Acquire));
        let logical_changed = pending.0 > 0 && pending.1 > 0 && pending != self.logical_size.get();
        if logical_changed {
            self.logical_size.set(pending);
        }

        // Host set_scale_factor writes the shared cell.
        let mut last = self.last_applied_scale.get();
        let scale_changed = self.scale.take_change(&mut last).is_some();
        let scale = self.scale.get();
        let (lw, lh) = self.logical_size.get();
        let (want_w, want_h) = Self::desired_physical(lw, lh, scale);

        let actual = self.window_ctx.size();
        let got_w = actual.physical.width;
        let got_h = actual.physical.height;
        let phys_mismatch = got_w != want_w || got_h != want_h;

        // Full geometry only when content scale or design logical size moved.
        // Physical mismatch alone → cheap child re-assert (throttled).
        if scale_changed || logical_changed {
            self.apply_geometry(lw, lh, scale);
        } else if phys_mismatch && n.is_multiple_of(15) {
            // ~4 Hz: enough to win after host layout, not every paint.
            self.resize_child_physical(want_w, want_h);
        }

        // Host frame push-back (non-Linux only — see host_resize_pushback_allowed).
        let host_correct = self.pending_host_correct.take();
        let retry = phys_mismatch && n.is_multiple_of(30); // ~0.5s at 60fps
        if Self::host_resize_pushback_allowed() && (host_correct.is_some() || retry) {
            if let Some(ref req) = self.request_resize {
                let _ = req(lw, lh);
            }
            self.resize_child_physical(want_w, want_h);
        }
    }

    fn init_policy_fields(
        policy: SizePolicy,
        request_resize: Option<RequestResizeFn>,
        open_scale: f64,
    ) -> PolicyFields {
        #[allow(clippy::cast_possible_truncation)]
        let scale_f32 = open_scale as f32;
        // Content scale is host/policy — never seed from OS DPI here when
        // host-driven (plugin path). Standalone may pass host_driven=false.
        if !policy.host_driven_scale {
            policy.scale.set(open_scale);
        }
        (
            policy.scale,
            Cell::new(scale_f32),
            policy.host_driven_scale,
            Cell::new(policy.design_size),
            policy.pending_size,
            Cell::new(None),
            request_resize,
            Cell::new(0),
        )
    }

    #[cfg(all(
        feature = "backend-femtovg",
        not(feature = "backend-skia"),
        not(feature = "backend-wgpu")
    ))]
    fn new<B>(
        window: &WindowContext,
        mut state: S,
        update: U,
        build: B,
        policy: SizePolicy,
        request_resize: Option<RequestResizeFn>,
    ) -> Result<SlintWindow<C, S, U>, HandlerError>
    where
        B: FnOnce(&mut S) -> C + Send + 'static,
    {
        // Soft-fail: missing GL context (old/broken GLX, no 3.2 Core) — do not panic.
        let Some(gl_context) = window.gl_context() else {
            return Err(HandlerError::from(
                crate::baseview_slint_window_adapter::GlInitError::new(
                    "LX UI: OpenGL 3.2 Core unavailable or FemtoVG init failed \
                     (no OpenGL context on window — editor left closed)",
                ),
            ));
        };
        if let Err(e) = unsafe { gl_context.make_current() } {
            return Err(HandlerError::from(
                crate::baseview_slint_window_adapter::GlInitError::new(format!(
                    "LX UI: OpenGL make_current failed ({e}) — editor left closed"
                )),
            ));
        }

        let open_scale = if policy.host_driven_scale {
            policy.scale.get()
        } else {
            window.size().scale_factor
        };
        let (lw, lh) = policy.design_size;
        let phys_w = to_physical_px(lw, open_scale);
        let phys_h = to_physical_px(lh, open_scale);

        // One-shot set_platform + per-open adapter handoff (reopen-safe).
        platform::ensure_platform();
        let adapter = match BaseviewSlintWindowAdapter::try_new(
            PhysicalSize::new(phys_w, phys_h),
            gl_context.clone(),
        ) {
            Ok(a) => a,
            Err(e) => {
                let _ = unsafe { gl_context.make_not_current() };
                return Err(HandlerError::from(e));
            }
        };
        platform::set_next_adapter(adapter.clone() as Rc<dyn slint::platform::WindowAdapter>);

        let component = build(&mut state);

        // Announce open-time scale + logical size (truce-slint parity).
        #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
        {
            adapter.slint_window.dispatch_event(
                slint::platform::WindowEvent::ScaleFactorChanged {
                    scale_factor: open_scale as f32,
                },
            );
            adapter
                .slint_window
                .dispatch_event(slint::platform::WindowEvent::Resized {
                    size: LogicalSize::new(lw as f32, lh as f32),
                });
        }

        let _ = unsafe { gl_context.make_not_current() };

        // Force child HWND to content physical size (not OS-DPI logical).
        let _ = window.resize(dpi::PhysicalSize::new(phys_w, phys_h));

        let (
            scale,
            last_applied_scale,
            host_driven_scale,
            logical_size,
            pending_size,
            pending_host_correct,
            request_resize,
            frames,
        ) = Self::init_policy_fields(policy, request_resize, open_scale);

        Ok(SlintWindow {
            update,
            last_cursor_pos: RefCell::new(LogicalPosition::new(0.0, 0.0)),
            component,
            state: RefCell::new(state),
            adapter: adapter.clone(),
            window_ctx: window.clone(),
            key_capture: KeyCapture::default(),
            pending_clipboard_paste: Cell::new(false),
            ctrl_held: Cell::new(false),
            scale,
            last_applied_scale,
            host_driven_scale,
            logical_size,
            pending_size,
            pending_host_correct,
            request_resize,
            frames,
        })
    }

    #[cfg(all(
        feature = "backend-skia",
        not(feature = "backend-femtovg"),
        not(feature = "backend-wgpu")
    ))]
    fn new<B>(
        window: &WindowContext,
        mut state: S,
        update: U,
        build: B,
        policy: SizePolicy,
        request_resize: Option<RequestResizeFn>,
    ) -> Result<SlintWindow<C, S, U>, HandlerError>
    where
        B: FnOnce(&mut S) -> C + Send + 'static,
    {
        let open_scale = if policy.host_driven_scale {
            policy.scale.get()
        } else {
            window.size().scale_factor
        };
        let (lw, lh) = policy.design_size;
        let phys_w = to_physical_px(lw, open_scale);
        let phys_h = to_physical_px(lh, open_scale);

        platform::ensure_platform();
        let adapter =
            SkiaWindowAdapter::new(PhysicalSize::new(phys_w, phys_h), window);
        platform::set_next_adapter(adapter.clone() as Rc<dyn slint::platform::WindowAdapter>);

        let component = build(&mut state);

        #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
        {
            adapter.slint_window.dispatch_event(
                slint::platform::WindowEvent::ScaleFactorChanged {
                    scale_factor: open_scale as f32,
                },
            );
            adapter
                .slint_window
                .dispatch_event(slint::platform::WindowEvent::Resized {
                    size: LogicalSize::new(lw as f32, lh as f32),
                });
        }

        let _ = window.resize(dpi::PhysicalSize::new(phys_w, phys_h));

        let (
            scale,
            last_applied_scale,
            host_driven_scale,
            logical_size,
            pending_size,
            pending_host_correct,
            request_resize,
            frames,
        ) = Self::init_policy_fields(policy, request_resize, open_scale);

        Ok(SlintWindow {
            update,
            last_cursor_pos: RefCell::new(LogicalPosition::new(0.0, 0.0)),
            component,
            state: RefCell::new(state),
            adapter: adapter.clone(),
            window_ctx: window.clone(),
            key_capture: KeyCapture::default(),
            pending_clipboard_paste: Cell::new(false),
            ctrl_held: Cell::new(false),
            scale,
            last_applied_scale,
            host_driven_scale,
            logical_size,
            pending_size,
            pending_host_correct,
            request_resize,
            frames,
        })
    }

    #[cfg(all(
        feature = "backend-wgpu",
        not(feature = "backend-femtovg"),
        not(feature = "backend-skia")
    ))]
    fn new<B>(
        window: &WindowContext,
        mut state: S,
        update: U,
        build: B,
        policy: SizePolicy,
        request_resize: Option<RequestResizeFn>,
    ) -> Result<SlintWindow<C, S, U>, HandlerError>
    where
        B: FnOnce(&mut S) -> C + Send + 'static,
    {
        let open_scale = if policy.host_driven_scale {
            policy.scale.get()
        } else {
            window.size().scale_factor
        };
        let (lw, lh) = policy.design_size;
        let phys_w = to_physical_px(lw, open_scale);
        let phys_h = to_physical_px(lh, open_scale);

        platform::ensure_platform();
        let slint_window = MinimalSoftwareWindow::new(
            slint::platform::software_renderer::RepaintBufferType::NewBuffer,
        );
        platform::set_next_adapter(slint_window.clone() as Rc<dyn slint::platform::WindowAdapter>);

        let component = build(&mut state);

        #[allow(clippy::cast_possible_truncation)]
        {
            slint_window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged {
                scale_factor: open_scale as f32,
            });
            slint_window.set_size(slint::WindowSize::Physical(PhysicalSize::new(
                phys_w, phys_h,
            )));
            slint_window.dispatch_event(slint::platform::WindowEvent::Resized {
                size: LogicalSize::new(lw as f32, lh as f32),
            });
        }

        let (device, queue, surface, surface_config) = match create_wgpu_surface(window) {
            Ok(s) => s,
            Err(e) => {
                return Err(HandlerError::from(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("LX UI: wgpu surface init failed ({e}) — editor left closed"),
                )));
            }
        };

        let blit = BlitPipeline::new(&device, surface_config.format, phys_w, phys_h);

        let _ = window.resize(dpi::PhysicalSize::new(phys_w, phys_h));

        let (
            scale,
            last_applied_scale,
            host_driven_scale,
            logical_size,
            pending_size,
            pending_host_correct,
            request_resize,
            frames,
        ) = Self::init_policy_fields(policy, request_resize, open_scale);

        Ok(SlintWindow {
            update,
            last_cursor_pos: RefCell::new(LogicalPosition::new(0.0, 0.0)),
            component,
            state: RefCell::new(state),
            wgpu: WgpuState {
                slint_window,
                device,
                queue,
                surface,
                surface_config: RefCell::new(surface_config),
                blit: RefCell::new(blit),
                px_buf: RefCell::new(Vec::new()),
                rgba_buf: RefCell::new(Vec::new()),
            },
            window_ctx: window.clone(),
            key_capture: KeyCapture::default(),
            pending_clipboard_paste: Cell::new(false),
            ctrl_held: Cell::new(false),
            scale,
            last_applied_scale,
            host_driven_scale,
            logical_size,
            pending_size,
            pending_host_correct,
            request_resize,
            frames,
        })
    }

    /// Open a parented window with the default [`SizePolicy`].
    ///
    /// # Errors
    /// Returns `Err` when the OS window cannot be created or shown.
    pub fn open_parented<P, B>(
        parent: &P,
        options: WindowSettings,
        state: S,
        build: B,
        update: U,
    ) -> Result<Window, baseview::Error>
    where
        B: FnOnce(&mut S) -> C,
        B: 'static + Send,
        P: HasWindowHandle,
    {
        Self::open_parented_with_policy(
            parent,
            options,
            state,
            build,
            update,
            SizePolicy::default(),
            None,
        )
    }

    /// Open a parented window with `HiDPI` / size policy (truce-slint parity).
    ///
    /// # Errors
    /// Returns `Err` when the OS window cannot be created or shown.
    pub fn open_parented_with_policy<P, B>(
        parent: &P,
        options: WindowSettings,
        state: S,
        build: B,
        update: U,
        policy: SizePolicy,
        request_resize: Option<RequestResizeFn>,
    ) -> Result<Window, baseview::Error>
    where
        B: FnOnce(&mut S) -> C,
        B: 'static + Send,
        P: HasWindowHandle,
    {
        let options = options.with_parent(Some(parent));
        let window = Window::create(options, move |w| {
            // FemtoVG path: Result (soft-fail on GL). Other backends: always Ok.
            SlintWindow::new(&w, state, update, build, policy, request_resize)
        })?;
        window.show()?;
        Ok(window)
    }

    /// Open a standalone window and run the event loop until it closes.
    ///
    /// # Errors
    /// Returns `Err` when the OS window cannot be created.
    pub fn open_blocking<B>(
        options: WindowSettings,
        state: S,
        build: B,
        update: U,
    ) -> Result<(), baseview::Error>
    where
        B: FnOnce(&mut S) -> C + Send + 'static,
    {
        let window = Window::create(options, move |w| {
            SlintWindow::new(
                &w,
                state,
                update,
                build,
                SizePolicy::default(),
                None,
            )
        })?;
        window.run_until_closed()
    }

    #[cfg(any(
        feature = "backend-femtovg",
        feature = "backend-skia",
        feature = "backend-wgpu"
    ))]
    fn flush_pending_clipboard_paste(&self) {
        if self.pending_clipboard_paste.replace(false)
            && let Some(clip) = crate::platform::clipboard_get_retry()
        {
            inject_clipboard_text(self.slint_window_ref(), &clip);
        }
    }

    pub(crate) fn handle_resize(&self, new_size: &WindowSize) {
        // Standalone may mirror OS DPI into content scale. Plugin path
        // (host_driven_scale): content scale is host set_scale only (default
        // 1.0) so child pixels match CLAP/VST3 get_size = size() * host_scale.
        // Never adopt baseview's OS Xft.dpi here on the plugin path — that
        // desyncs mouse (physical / content_scale) from the rendered layout.
        if !self.host_driven_scale {
            self.scale.set(new_size.scale_factor);
        }

        let (lw, lh) = self.logical_size.get();
        let scale = self.scale.get();
        let (want_w, want_h) = Self::desired_physical(lw, lh, scale);
        let got_w = new_size.physical.width;
        let got_h = new_size.physical.height;

        // Never reflow from host HWND size. Re-assert our physical pixel size.
        // Host push-back is deferred to reconcile_pending (and skipped on Linux).
        if got_w != want_w || got_h != want_h {
            self.resize_child_physical(want_w, want_h);
            if Self::host_resize_pushback_allowed() {
                self.queue_host_correct((lw, lh));
            }
        }
    }

    pub fn window(&self) -> &slint::Window {
        self.slint_window_ref()
    }
}

impl<C, S, U> baseview::WindowHandler for SlintWindow<C, S, U>
where
    C: ComponentHandle,
    S: Send + 'static,
    U: Fn(&C, &mut S) + Send + 'static,
{
    #[cfg(all(feature = "backend-femtovg", not(feature = "backend-skia"), not(feature = "backend-wgpu")))]
    fn on_frame(&self) -> Result<(), HandlerError> {
        slint::platform::update_timers_and_animations();
        self.flush_pending_clipboard_paste();
        // HiDPI / size reconcile before author sync + paint (truce-slint).
        self.reconcile_pending();
        (self.update)(&self.component, &mut *self.state.borrow_mut());
        self.adapter.renderer.render().map_err(HandlerError::from)
    }

    #[cfg(all(feature = "backend-skia", not(feature = "backend-femtovg"), not(feature = "backend-wgpu")))]
    fn on_frame(&self) -> Result<(), HandlerError> {
        slint::platform::update_timers_and_animations();
        self.flush_pending_clipboard_paste();
        self.reconcile_pending();
        (self.update)(&self.component, &mut *self.state.borrow_mut());
        self.adapter.renderer.render().map(|_| ()).map_err(HandlerError::from)
    }

    #[cfg(all(feature = "backend-wgpu", not(feature = "backend-femtovg"), not(feature = "backend-skia")))]
    fn on_frame(&self) -> Result<(), HandlerError> {
        slint::platform::update_timers_and_animations();
        self.flush_pending_clipboard_paste();
        self.reconcile_pending();
        (self.update)(&self.component, &mut *self.state.borrow_mut());

        let sc = self.wgpu.surface_config.borrow();
        let (surf_w, surf_h) = (sc.width, sc.height);
        drop(sc);

        self.wgpu.slint_window.request_redraw();
        render_to_rgba(
            &self.wgpu.slint_window,
            surf_w,
            surf_h,
            &mut self.wgpu.px_buf.borrow_mut(),
            &mut self.wgpu.rgba_buf.borrow_mut(),
        );

        let surface_texture = match self.wgpu.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(tex) => tex,
            wgpu::CurrentSurfaceTexture::Suboptimal(tex) => tex,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            e => {
                return Err(string_err(format!(
                    "get_current_texture: {e:?}"
                )));
            }
        };

        let rgba = self.wgpu.rgba_buf.borrow();
        self.wgpu.blit.borrow().update(&self.wgpu.queue, &rgba);
        drop(rgba);

        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.wgpu.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor {
                label: Some("slint-wgpu-frame"),
            },
        );
        let sc = self.wgpu.surface_config.borrow();
        self.wgpu.blit.borrow().render(
            &self.wgpu.queue,
            &mut encoder,
            &view,
            sc.width,
            sc.height,
        );
        drop(sc);
        self.wgpu.queue.submit(std::iter::once(encoder.finish()));
        self.wgpu.queue.present(surface_texture);

        Ok(())
    }

    // ponytail: one big event-dispatch match; splitting it hurts readability.
    #[allow(clippy::too_many_lines)]
    fn on_event(&self, event: Event) -> EventStatus {
        match event {
            Event::Mouse(mouse_event) => {
                let slint_mouse_event = match mouse_event {
                    baseview::MouseEvent::CursorMoved { position, .. } => {
                        // baseview (Win/X11/macOS) delivers child-window pixels
                        // (physical). Slint Pointer* wants logical points =
                        // physical / content scale (host set_scale, default 1.0).
                        // Must use content scale, NOT baseview OS Xft.dpi — those
                        // diverge on Linux HiDPI and make knobs "fight" the cursor.
                        let s = self.last_applied_scale.get().max(0.001);
                        #[allow(clippy::cast_possible_truncation)]
                        let pos = LogicalPosition::new(
                            position.x as f32 / s,
                            position.y as f32 / s,
                        );
                        *self.last_cursor_pos.borrow_mut() = pos;
                        WindowEvent::PointerMoved { position: pos }
                    }
                    baseview::MouseEvent::ButtonPressed { button, .. } => {
                        // WS_CHILD plugin windows don't receive WM_KEYDOWN
                        // until focused; baseview doesn't SetFocus on click,
                        // so we do it here. Without this, TextInput never sees
                        // keystrokes on Windows (truce-slint parity).
                        #[cfg(target_os = "windows")]
                        if !self.window_ctx.has_focus() {
                            let _ = self.window_ctx.focus();
                        }
                        let slint_button = translate_mouse_button(button);
                        WindowEvent::PointerPressed {
                            button: slint_button,
                            position: *self.last_cursor_pos.borrow(),
                        }
                    }
                    baseview::MouseEvent::ButtonReleased { button, .. } => {
                        let slint_button = translate_mouse_button(button);
                        WindowEvent::PointerReleased {
                            button: slint_button,
                            position: *self.last_cursor_pos.borrow(),
                        }
                    }
                    baseview::MouseEvent::WheelScrolled { delta, .. } => {
                        let (delta_x, delta_y) = match delta {
                            baseview::ScrollDelta::Lines { x, y } => (x * 20.0, y * 20.0),
                            baseview::ScrollDelta::Pixels { x, y } => (x, y),
                        };
                        WindowEvent::PointerScrolled {
                            position: LogicalPosition::new(0.0, 0.0),
                            delta_x,
                            delta_y,
                        }
                    }
                    _ => return EventStatus::Ignored,
                };

                self.slint_window_ref().dispatch_event(slint_mouse_event);
                EventStatus::Captured
            }
            Event::Keyboard(key_event) => {
                use keyboard_types::{Key as KK, KeyState, Modifiers, NamedKey};
                use slint::platform::Key as SK;

                let win = self.slint_window_ref();
                let mods = key_event.modifiers;
                let is_down = matches!(key_event.state, KeyState::Down);
                let is_up = matches!(key_event.state, KeyState::Up);

                // Track Control ourselves: hosts may drop the modifier flag
                // on the character event even while Control is physically held.
                if matches!(&key_event.key, KK::Named(NamedKey::Control)) {
                    self.ctrl_held.set(is_down);
                }
                let ctrl = mods.contains(Modifiers::CONTROL) || self.ctrl_held.get();

                let is_modifier = matches!(
                    &key_event.key,
                    KK::Named(
                        NamedKey::Control
                            | NamedKey::Shift
                            | NamedKey::Alt
                            | NamedKey::Meta
                            | NamedKey::AltGraph
                            | NamedKey::CapsLock
                    )
                );

                // ── Ctrl+V / Paste: capture and defer to on_frame ───────────
                // Reading the OS clipboard inside WM_KEYDOWN/WM_CHAR fails
                // re-entrantly: the host has the clipboard open while the key
                // message is in flight. Set a flag and read it on the next
                // frame, when the host has closed the clipboard.
                let is_paste_key = match &key_event.key {
                    KK::Named(NamedKey::Paste) => true,
                    KK::Character(s) if s.eq_ignore_ascii_case("v") => true,
                    _ if key_event.code == Code::KeyV => true,
                    _ => false,
                };
                if is_down
                    && !key_event.repeat
                    && is_paste_key
                    && (ctrl || matches!(&key_event.key, KK::Named(NamedKey::Paste)))
                {
                    self.pending_clipboard_paste.set(true);
                    return EventStatus::Captured;
                }

                // Sync Control from event flags (hosts drop pure Control keys).
                if !is_modifier && is_down {
                    if ctrl {
                        win.dispatch_event(WindowEvent::KeyPressed {
                            text: SK::Control.into(),
                        });
                    } else {
                        win.dispatch_event(WindowEvent::KeyReleased {
                            text: SK::Control.into(),
                        });
                    }
                }

                // Ctrl+C / Ctrl+X still go through StandardShortcut + Platform.
                let text: Option<slint::SharedString> = match &key_event.key {
                    KK::Named(NamedKey::Copy) if is_down => {
                        win.dispatch_event(WindowEvent::KeyPressed {
                            text: SK::Control.into(),
                        });
                        Some("c".into())
                    }
                    KK::Named(NamedKey::Cut) if is_down => {
                        win.dispatch_event(WindowEvent::KeyPressed {
                            text: SK::Control.into(),
                        });
                        Some("x".into())
                    }
                    KK::Character(s) if mods.contains(Modifiers::CONTROL) => {
                        Some(s.to_lowercase().into())
                    }
                    other => crate::translate::translate_virtual_key(other),
                };

                if let Some(key) = text {
                    let e = match (key_event.state, key_event.repeat) {
                        (KeyState::Down, true) => WindowEvent::KeyPressRepeated { text: key },
                        (KeyState::Down, false) => WindowEvent::KeyPressed { text: key },
                        (KeyState::Up, _) => WindowEvent::KeyReleased { text: key },
                    };
                    win.dispatch_event(e);
                }

                let _ = is_up;

                match &self.key_capture {
                    KeyCapture::CaptureAll => EventStatus::Captured,
                    KeyCapture::IgnoreAll => EventStatus::Ignored,
                    KeyCapture::CaptureKeys(keys) => {
                        if keys.contains(&key_event.key) {
                            EventStatus::Captured
                        } else {
                            EventStatus::Ignored
                        }
                    }
                    KeyCapture::IgnoreKeys(keys) => {
                        if keys.contains(&key_event.key) {
                            EventStatus::Ignored
                        } else {
                            EventStatus::Captured
                        }
                    }
                }
            }
            _ => EventStatus::Ignored,
        }
    }

    fn resized(&self, new_size: baseview::WindowSize) -> Result<(), HandlerError> {
        self.handle_resize(&new_size);
        Ok(())
    }
}

/// Paste into the focused `TextInput` without `Platform::clipboard_text`:
/// clear Control, Select-All, then type each character.
fn inject_clipboard_text(win: &slint::Window, text: &str) {
    use slint::platform::Key as SK;
    // Ensure Control is not held (would turn letters into shortcuts).
    win.dispatch_event(WindowEvent::KeyReleased {
        text: SK::Control.into(),
    });
    // Select all so we replace the current field contents (path fields).
    win.dispatch_event(WindowEvent::KeyPressed {
        text: SK::Control.into(),
    });
    win.dispatch_event(WindowEvent::KeyPressed {
        text: "a".into(),
    });
    win.dispatch_event(WindowEvent::KeyReleased {
        text: SK::Control.into(),
    });
    for ch in text.chars() {
        // Skip control chars (CR/LF from multi-line copies).
        if ch.is_control() {
            continue;
        }
        let t: slint::SharedString = ch.to_string().into();
        win.dispatch_event(WindowEvent::KeyPressed { text: t.clone() });
        win.dispatch_event(WindowEvent::KeyReleased { text: t });
    }
}

// --- wgpu surface creation ---

#[cfg(feature = "backend-wgpu")]
fn string_err(s: impl Into<String>) -> HandlerError {
    HandlerError::from(std::io::Error::new(std::io::ErrorKind::Other, s.into()))
}

#[cfg(feature = "backend-wgpu")]
fn create_wgpu_surface(
    window: &WindowContext,
) -> Result<
    (
        wgpu::Device,
        wgpu::Queue,
        wgpu::Surface<'static>,
        wgpu::SurfaceConfiguration,
    ),
    String,
> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        #[cfg(feature = "backend-wgpu-vulkan")]
        backends: wgpu::Backends::VULKAN,
        #[cfg(not(feature = "backend-wgpu-vulkan"))]
        backends: wgpu::Backends::all(),
        flags: wgpu::InstanceFlags::default(),
        memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
        backend_options: wgpu::BackendOptions::default(),
        display: None,
    });

    // SAFETY: WindowContext is valid for the handler's lifetime.
    let surface = unsafe {
        let rwh = window
            .window_handle()
            .map_err(|e| format!("window_handle: {e}"))?;
        let rdh = window
            .display_handle()
            .map_err(|e| format!("display_handle: {e}"))?;
        instance
            .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(rdh.as_raw()),
                raw_window_handle: rwh.as_raw(),
            })
    }
    .map_err(|e| format!("create_surface: {e}"))?;

    let adapter = pollster::block_on(instance.request_adapter(
        &wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        },
    ))
    .map_err(|e| format!("no wgpu adapter: {e}"))?;

    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("slint-baseview-wgpu"),
            required_features: wgpu::Features::empty(),
            required_limits: adapter.limits(),
            experimental_features: wgpu::ExperimentalFeatures::default(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .map_err(|e| format!("request_device: {e}"))?;

    let caps = surface.get_capabilities(&adapter);
    let format = caps
        .formats
        .iter()
        .find(|f| f.is_srgb())
        .copied()
        .unwrap_or(caps.formats[0]);

    let phys = window.size().physical;
    let surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: phys.width.max(1),
        height: phys.height.max(1),
        #[cfg(target_os = "windows")]
        present_mode: wgpu::PresentMode::AutoNoVsync,
        #[cfg(not(target_os = "windows"))]
        present_mode: wgpu::PresentMode::AutoVsync,
        desired_maximum_frame_latency: 2,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        view_formats: vec![],
        color_space: wgpu::SurfaceColorSpace::Auto,
    };
    surface.configure(&device, &surface_config);

    Ok((device, queue, surface, surface_config))
}

