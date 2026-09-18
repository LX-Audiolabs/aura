//! Skia-based window adapter for slint-baseview.
//!
//! Wraps `SkiaRenderer` (Direct3D on Windows, Metal on macOS, OpenGL on Linux)
//! instead of FemtoVG OpenGL. No GL context from baseview needed — Skia
//! uses platform-native GPU APIs directly.
//!
//! Pattern mirrors `BaseviewSlintWindowAdapter` — same `WindowAdapter` trait,
//! different renderer.

use std::{cell::RefCell, rc::Rc, sync::Arc};

use crate::owned_handles::OwnedWindowHandles;
use i_slint_renderer_skia::{SkiaRenderer, SkiaSharedContext};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use slint::{PhysicalSize, Window, platform::WindowAdapter};

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

        let handles =
            Arc::new(OwnedWindowHandles::new(window_handle).expect("window/display handle"));

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
