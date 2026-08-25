use baseview::gl::GlContext;
use slint::platform::femtovg_renderer::OpenGLInterface;
use std::ffi::CStr;
use std::sync::atomic::{AtomicBool, Ordering};

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

/// Soft-fail if the current context is the Windows GL 1.1 software stub.
///
/// # Safety
/// Caller must have made `gl` current on this thread.
pub(crate) unsafe fn reject_software_gl11(gl: &GlContext) -> Result<(), String> {
    const GL_VERSION: u32 = 0x1F02;
    type GlGetString = unsafe extern "system" fn(u32) -> *const std::ffi::c_char;

    let proc = gl.get_proc_address_from_str("glGetString");
    if proc.is_null() {
        return Err(
            "glGetString unavailable (null proc) — broken WGL load / no real GL driver".into(),
        );
    }
    // SAFETY: get_proc_address returned a symbol; context is current.
    let gl_get_string: GlGetString = unsafe { std::mem::transmute(proc) };
    let version = unsafe { cstr_or_q(gl_get_string(GL_VERSION)) };
    if version.starts_with("1.1") {
        return Err(format!(
            "OpenGL {version} (Windows software loader) — FemtoVG needs 3.2 Core"
        ));
    }
    Ok(())
}

unsafe fn cstr_or_q(ptr: *const std::ffi::c_char) -> String {
    if ptr.is_null() {
        return "?".into();
    }
    // SAFETY: GL returns a static NUL-terminated string for these enums.
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

unsafe impl OpenGLInterface for SlintGlContext {
    fn ensure_current(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let gl_context = &self.gl_context;
        if let Err(e) = unsafe { gl_context.make_current() } {
            // Must NOT be reported as Err. Slint's `unregister_item_tree` does
            // `renderer().free_graphics_resources(..).expect(..)`, and that runs
            // from the Slint component's `Drop` — which we reach from inside the
            // Win32 `WM_DESTROY` window procedure. Hosts that tear the parent
            // HWND down before calling `gui_destroy` (Bitwig's plugin sandbox)
            // leave the child's CS_OWNDC HDC dead, so `wglMakeCurrent` fails
            // with ERROR_INVALID_HANDLE, Slint panics, and the panic aborts at
            // the `extern "system"` boundary — killing the whole host/sandbox
            // process (exit 0xC000041D). `catch_unwind` around `Editor::close`
            // cannot help: the abort happens before the unwind leaves wnd_proc.
            //
            // With no context current the follow-up `glDelete*` calls are
            // no-ops, and every GL object dies anyway with `wglDeleteContext`
            // when the window's `GlContext` drops — so nothing leaks.
            // Genuine init failures are still caught: `SlintWindow::new` calls
            // `make_current` directly and soft-fails the whole editor there.
            //
            // Warn once per process: on a lost device this is hit from the
            // render path on *every* frame, and 60 lines/s into the host's
            // stderr is its own bug.
            static WARNED: AtomicBool = AtomicBool::new(false);
            if !WARNED.swap(true, Ordering::Relaxed) {
                eprintln!(
                    "[AURA UI] GL make_current failed ({e}) — continuing without a current \
                     context (window teardown or lost device)"
                );
            }
        }
        Ok(())
    }

    fn swap_buffers(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let gl_context = &self.gl_context;
        if let Err(e) = gl_context.swap_buffers() {
            return Err(format!("swap_buffers failed: {e}").into());
        }
        // Keep context current on the GUI thread. Releasing every frame
        // (make_not_current) fights DAW embeds and some Win11 drivers.
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
