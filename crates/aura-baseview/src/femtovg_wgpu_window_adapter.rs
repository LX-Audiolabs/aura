//! FemtoVG + wgpu window adapter for aura-baseview.
//!
//! Renders via Slint's `FemtoVGRenderer<WGPUBackend>` directly to a wgpu
//! surface created from the baseview raw-window-handle — same idea as Slint's
//! winit `WGPUFemtoVGRenderer`, not the software-blit path.

use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use crate::init_error::InitError;
use crate::owned_handles::OwnedWindowHandles;
use i_slint_renderer_femtovg::{FemtoVGRenderer, FemtoVGRendererExt, wgpu::WGPUBackend};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use slint::{PhysicalSize, Window, platform::WindowAdapter};

pub struct FemtovgWgpuWindowAdapter {
    pub renderer: FemtoVGRenderer<WGPUBackend>,
    pub slint_window: Window,
    physical_size: RefCell<PhysicalSize>,
}

impl FemtovgWgpuWindowAdapter {
    /// Create FemtoVG+wgpu adapter. Returns [`Err`] instead of panicking when
    /// surface / device init fails.
    ///
    /// # Errors
    /// Returns [`InitError`] when wgpu surface setup or FemtoVG init fails.
    pub fn try_new(
        physical_size: PhysicalSize,
        window_handle: &(impl HasWindowHandle + HasDisplayHandle),
    ) -> Result<Rc<Self>, InitError> {
        let handles: Box<dyn wgpu::DisplayAndWindowHandle + 'static> = Box::new(
            OwnedWindowHandles::new(window_handle)
                .map_err(|e| InitError::new(format!("window/display handle: {e}")))?,
        );

        let adapter = Rc::new_cyclic(|weak_self| {
            let renderer = FemtoVGRenderer::<WGPUBackend>::new_suspended();
            let slint_window = Window::new(weak_self.clone() as _);
            Self {
                renderer,
                slint_window,
                physical_size: RefCell::new(physical_size),
            }
        });

        // Prefer Result from set_surface; also catch panics from broken drivers.
        let set_result = catch_unwind(AssertUnwindSafe(|| {
            adapter
                .renderer
                .set_surface(handles, physical_size, None, false)
        }));

        match set_result {
            Ok(Ok(())) => Ok(adapter),
            Ok(Err(e)) => Err(InitError::new(format!(
                "LX UI: FemtoVG wgpu surface init failed ({e})"
            ))),
            Err(_) => Err(InitError::new("LX UI: FemtoVG wgpu surface init panicked")),
        }
    }

    pub fn update_size(&self, physical_size: PhysicalSize) {
        *self.physical_size.borrow_mut() = physical_size;
    }
}

impl WindowAdapter for FemtovgWgpuWindowAdapter {
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
