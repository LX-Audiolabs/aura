//! Skia-based window adapter for slint-baseview.
//!
//! Wraps `SkiaRenderer` (Direct3D on Windows, Metal on macOS, OpenGL on Linux)
//! instead of FemtoVG OpenGL. No GL context from baseview needed — Skia
//! uses platform-native GPU APIs directly.
//!
//! Pattern mirrors `BaseviewSlintWindowAdapter` — same `WindowAdapter` trait,
//! different renderer.

use std::{cell::RefCell, rc::Rc, sync::Arc};

use i_slint_renderer_skia::{SkiaRenderer, SkiaSharedContext};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WindowHandle,
};
use slint::{platform::WindowAdapter, PhysicalSize, Window};

/// Owned window handle pair extracted from `baseview::WindowContext`.
///
/// `i-slint-renderer-skia` requires `Arc<dyn HasWindowHandle + Send + Sync>`,
/// but `baseview::WindowContext` is `Rc`-based (not `Send`). We extract raw
/// handles which are immutable integers/pointers — safe to share.
struct OwnedWindowHandles {
    raw_window: RawWindowHandle,
    raw_display: RawDisplayHandle,
}

// SAFETY: Raw window/display handles are platform handle integers that remain
// valid for the adapter's lifetime. They are never mutated after extraction.
unsafe impl Send for OwnedWindowHandles {}
unsafe impl Sync for OwnedWindowHandles {}

impl HasWindowHandle for OwnedWindowHandles {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        // SAFETY: raw handle extracted from valid WindowContext, valid for adapter lifetime.
        Ok(unsafe { WindowHandle::borrow_raw(self.raw_window) })
    }
}

impl HasDisplayHandle for OwnedWindowHandles {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        // SAFETY: raw handle extracted from valid WindowContext, valid for adapter lifetime.
        Ok(unsafe { DisplayHandle::borrow_raw(self.raw_display) })
    }
}

pub struct SkiaWindowAdapter {
    pub renderer: SkiaRenderer,
    pub slint_window: Window,
    physical_size: RefCell<PhysicalSize>,
}

impl SkiaWindowAdapter {
    /// Create a new Skia-backed window adapter.
    ///
    /// `window_handle` must implement `HasWindowHandle + HasDisplayHandle`
    /// (typically `baseview::WindowContext`). Raw handles are extracted and
    /// wrapped in `Arc` for the Skia renderer.
    pub fn new(
        physical_size: PhysicalSize,
        window_handle: &(impl HasWindowHandle + HasDisplayHandle),
    ) -> Rc<Self> {
        let skia_context = SkiaSharedContext::default();

        // Platform-optimal GPU backend — same pattern as plugin-canvas-slint.
        #[cfg(target_os = "windows")]
        let renderer = SkiaRenderer::default_direct3d(&skia_context);
        #[cfg(target_os = "macos")]
        let renderer = SkiaRenderer::default_metal(&skia_context);
        #[cfg(target_os = "linux")]
        let renderer = SkiaRenderer::default_opengl(&skia_context);

        let handles = {
            let wh = window_handle.window_handle().expect("window_handle");
            let dh = window_handle.display_handle().expect("display_handle");
            Arc::new(OwnedWindowHandles {
                raw_window: wh.as_raw(),
                raw_display: dh.as_raw(),
            })
        };

        let wh: Arc<dyn HasWindowHandle + Send + Sync> = handles.clone();
        let dh: Arc<dyn HasDisplayHandle + Send + Sync> = handles;
        renderer
            .set_window_handle(wh, dh, physical_size, None)
            .expect("Failed to set skia window handle");

        Rc::new_cyclic(|weak_self| {
            let slint_window = Window::new(weak_self.clone() as _);
            Self {
                renderer,
                slint_window,
                physical_size: RefCell::new(physical_size),
            }
        })
    }

    pub fn update_size(&self, physical_size: PhysicalSize) {
        *self.physical_size.borrow_mut() = physical_size;
    }
}

impl WindowAdapter for SkiaWindowAdapter {
    fn window(&self) -> &Window {
        &self.slint_window
    }

    fn size(&self) -> PhysicalSize {
        *self.physical_size.borrow()
    }

    fn renderer(&self) -> &dyn slint::platform::Renderer {
        &self.renderer
    }

    fn request_redraw(&self) {
        // baseview handles redraws in on_frame
    }
}
