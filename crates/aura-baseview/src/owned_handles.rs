use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WindowHandle,
};

/// Owned window/display handle pair extracted from `baseview::WindowContext`.
///
/// Skia and wgpu need `Send + Sync + 'static` handle providers, but
/// `baseview::WindowContext` is `Rc`-based and borrowed. This copies the raw
/// handles out so the renderer can hold them.
pub(crate) struct OwnedWindowHandles {
    raw_window: RawWindowHandle,
    raw_display: RawDisplayHandle,
}

impl OwnedWindowHandles {
    pub(crate) fn new(
        source: &(impl HasWindowHandle + HasDisplayHandle),
    ) -> Result<Self, HandleError> {
        Ok(Self {
            raw_window: source.window_handle()?.as_raw(),
            raw_display: source.display_handle()?.as_raw(),
        })
    }
}

// SAFETY: the wrapper is immutable after construction and only hands out the
// copied platform handle values (HWND / X11 id / NSView pointer). It never
// dereferences them. This covers `Send`/`Sync` of the value only: nothing here
// keeps the OS window alive, so the renderer holding these must be dropped
// before baseview destroys the window. That holds today because the adapter is
// owned by `SlintWindow` (the baseview handler), but it is not enforced by the
// type system or asserted.
unsafe impl Send for OwnedWindowHandles {}
unsafe impl Sync for OwnedWindowHandles {}

impl HasWindowHandle for OwnedWindowHandles {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        // SAFETY: see the `Send`/`Sync` note above; the handle came from a live
        // `WindowContext` at construction.
        Ok(unsafe { WindowHandle::borrow_raw(self.raw_window) })
    }
}

impl HasDisplayHandle for OwnedWindowHandles {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        // SAFETY: as in `window_handle`.
        Ok(unsafe { DisplayHandle::borrow_raw(self.raw_display) })
    }
}
