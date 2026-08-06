use baseview::gl::GlContext;
use slint::platform::femtovg_renderer::OpenGLInterface;

#[derive(Clone)]
pub struct SlintGlContext {
    gl_context: GlContext,
}

impl std::fmt::Debug for SlintGlContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BaseviewOpenGLInterface").finish()
    }
}

impl SlintGlContext {
    pub(crate) fn new(gl_context: GlContext) -> Self {
        Self { gl_context }
    }
}

unsafe impl OpenGLInterface for SlintGlContext {
    fn ensure_current(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let gl_context = &self.gl_context;
        if let Err(e) = unsafe { gl_context.make_current() } {
            return Err(format!("make_current failed: {e}").into());
        }
        Ok(())
    }

    fn swap_buffers(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let gl_context = &self.gl_context;
        if let Err(e) = gl_context.swap_buffers() {
            return Err(format!("swap_buffers failed: {e}").into());
        }
        if let Err(e) = unsafe { gl_context.make_not_current() } {
            return Err(format!("make_not_current failed: {e}").into());
        }
        Ok(())
    }

    fn resize(
        &self,
        _width: core::num::NonZeroU32,
        _height: core::num::NonZeroU32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Resize is handled via WindowAdapter::size()
        Ok(())
    }

    fn get_proc_address(&self, name: &core::ffi::CStr) -> *const core::ffi::c_void {
        let gl_context = &self.gl_context;
        gl_context.get_proc_address(name)
    }
}
