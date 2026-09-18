use crate::init_error::InitError;
use crate::open_gl_interface::SlintGlContext;
use baseview::gl::GlContext;
use slint::{
    PhysicalSize,
    platform::{WindowAdapter, femtovg_renderer::FemtoVGRenderer},
};
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

pub struct BaseviewSlintWindowAdapter {
    pub renderer: FemtoVGRenderer,
    pub slint_window: slint::Window,
    physical_size: RefCell<PhysicalSize>,
}

impl BaseviewSlintWindowAdapter {
    /// Create `FemtoVG` adapter. Returns [`Err`] instead of panicking when GL
    /// shaders / context are unavailable (old Linux/macOS hosts).
    ///
    /// # Errors
    /// Returns [`InitError`] when the OpenGL context or `FemtoVG` renderer
    /// init fails or panics.
    pub fn try_new(
        physical_size: PhysicalSize,
        gl_context: GlContext,
    ) -> Result<Rc<Self>, InitError> {
        let gl_interface = SlintGlContext::new(gl_context);

        // Prefer Result from FemtoVG; also catch panics from broken drivers.
        let renderer = match catch_unwind(AssertUnwindSafe(|| FemtoVGRenderer::new(gl_interface))) {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                return Err(InitError::new(format!(
                    "LX UI: OpenGL 3.2 Core unavailable or FemtoVG init failed ({e})"
                )));
            }
            Err(_) => {
                return Err(InitError::new(
                    "LX UI: OpenGL 3.2 Core unavailable or FemtoVG init panicked",
                ));
            }
        };

        Ok(Rc::new_cyclic(move |weak_self| {
            let slint_window = slint::Window::new(weak_self.clone() as _);
            Self {
                renderer,
                slint_window,
                physical_size: RefCell::new(physical_size),
            }
        }))
    }

    pub fn update_size(&self, physical_size: PhysicalSize) {
        *self.physical_size.borrow_mut() = physical_size;
    }
}

impl WindowAdapter for BaseviewSlintWindowAdapter {
    fn window(&self) -> &slint::Window {
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
