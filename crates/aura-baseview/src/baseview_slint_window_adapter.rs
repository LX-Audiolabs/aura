use crate::open_gl_interface::SlintGlContext;
use baseview::gl::GlContext;
use slint::{
    platform::{femtovg_renderer::FemtoVGRenderer, WindowAdapter},
    PhysicalSize,
};
use std::{
    cell::RefCell,
    fmt,
    panic::{catch_unwind, AssertUnwindSafe},
    rc::Rc,
};

/// Soft-fail error when OpenGL / FemtoVG cannot start (no host panic).
#[derive(Debug)]
pub struct GlInitError {
    pub message: String,
}

impl GlInitError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for GlInitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for GlInitError {}

pub struct BaseviewSlintWindowAdapter {
    pub renderer: FemtoVGRenderer,
    pub slint_window: slint::Window,
    physical_size: RefCell<PhysicalSize>,
}

impl BaseviewSlintWindowAdapter {
    /// Create FemtoVG adapter. Returns [`Err`] instead of panicking when GL
    /// shaders / context are unavailable (old Linux/macOS hosts).
    pub fn try_new(
        physical_size: PhysicalSize,
        gl_context: GlContext,
    ) -> Result<Rc<Self>, GlInitError> {
        let gl_interface = SlintGlContext::new(gl_context);

        // Prefer Result from FemtoVG; also catch panics from broken drivers.
        let renderer = match catch_unwind(AssertUnwindSafe(|| FemtoVGRenderer::new(gl_interface))) {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                return Err(GlInitError::new(format!(
                    "LX UI: OpenGL 3.2 Core unavailable or FemtoVG init failed ({e})"
                )));
            }
            Err(_) => {
                return Err(GlInitError::new(
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
