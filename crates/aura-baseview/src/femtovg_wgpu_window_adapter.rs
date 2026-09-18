//! FemtoVG + wgpu window adapter for aura-baseview.
//!
//! Renders via Slint's `FemtoVGRenderer<WGPUBackend>` directly to a wgpu
//! surface created from the baseview raw-window-handle — same idea as Slint's
//! winit `WGPUFemtoVGRenderer`, not the software-blit path.

use std::{
    cell::RefCell,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use i_slint_renderer_femtovg::{
    FemtoVGRenderer, FemtoVGRendererExt,
    wgpu::WGPUBackend,
};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WindowHandle,
};
use slint::{PhysicalSize, Window, platform::WindowAdapter};

/// Soft-fail error when FemtoVG/wgpu cannot start (no host panic).
#[derive(Debug)]
pub struct WgpuInitError {
    pub message: String,
}

impl WgpuInitError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for WgpuInitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for WgpuInitError {}

/// Owned window handle pair extracted from `baseview::WindowContext`.
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
    /// Returns [`WgpuInitError`] when wgpu surface setup or FemtoVG init fails.
    pub fn try_new(
        physical_size: PhysicalSize,
        window_handle: &(impl HasWindowHandle + HasDisplayHandle),
    ) -> Result<Rc<Self>, WgpuInitError> {
        let handles: Box<dyn wgpu::DisplayAndWindowHandle + 'static> = {
            let wh = window_handle
                .window_handle()
                .map_err(|e| WgpuInitError::new(format!("window_handle: {e}")))?;
            let dh = window_handle
                .display_handle()
                .map_err(|e| WgpuInitError::new(format!("display_handle: {e}")))?;
            Box::new(OwnedWindowHandles {
                raw_window: wh.as_raw(),
                raw_display: dh.as_raw(),
            })
        };

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
            Ok(Err(e)) => Err(WgpuInitError::new(format!(
                "LX UI: FemtoVG wgpu surface init failed ({e})"
            ))),
            Err(_) => Err(WgpuInitError::new(
                "LX UI: FemtoVG wgpu surface init panicked",
            )),
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
