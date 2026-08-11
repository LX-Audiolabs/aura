//! Parented VST3 editor (`IPlugView`) over the same [`Editor`] trait CLAP uses.
//!
//! Host flow: `createView("editor")` → `attached(parent, HWND|NSView|…)` →
//! `Editor::open` (baseview/Slint) → `removed` → `Editor::close`.

use std::ffi::{CStr, c_void};
use std::sync::{Arc, Mutex, PoisonError};

use aura_core::editor::{Editor, EditorBridge, PluginContext, RawWindowHandle};
use aura_params::Params;
use vst3::Steinberg::Vst::{IComponentHandler, IComponentHandlerTrait, ParamID, ParamValue};
#[cfg(target_os = "windows")]
use vst3::Steinberg::kPlatformTypeHWND;
#[cfg(target_os = "macos")]
use vst3::Steinberg::kPlatformTypeNSView;
#[cfg(target_os = "linux")]
use vst3::Steinberg::kPlatformTypeX11EmbedWindowID;
use vst3::Steinberg::{
    FIDString, IPlugFrame, IPlugView, IPlugViewTrait, TBool, ViewRect, char16, int16, int32,
    kInvalidArgument, kResultFalse, kResultOk, tresult,
};
use vst3::{Class, ComRef, ComWrapper};

// ---------------------------------------------------------------------------
// Shared GUI state (Component + PlugView)
// ---------------------------------------------------------------------------

/// Shared between the VST3 component and its `IPlugView` instance(s).
pub(crate) struct GuiState {
    pub editor: Mutex<Option<Box<dyn Editor>>>,
    pub params: Arc<dyn Params>,
    pub sample_rate: Mutex<f64>,
    /// Host `IComponentHandler` for begin/perform/endEdit (raw; host-owned).
    pub handler: Mutex<Option<*mut IComponentHandler>>,
    /// Host frame for resize (optional; raw; host-owned).
    pub frame: Mutex<Option<*mut IPlugFrame>>,
}

// SAFETY: host serializes GUI-thread access; raw pointers are only used there.
unsafe impl Send for GuiState {}
unsafe impl Sync for GuiState {}

impl GuiState {
    pub(crate) fn new(params: Arc<dyn Params>) -> Arc<Self> {
        Arc::new(Self {
            editor: Mutex::new(None),
            params,
            sample_rate: Mutex::new(44_100.0),
            handler: Mutex::new(None),
            frame: Mutex::new(None),
        })
    }

    fn lock_editor(&self) -> std::sync::MutexGuard<'_, Option<Box<dyn Editor>>> {
        self.editor.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn sample_rate(&self) -> f64 {
        *self
            .sample_rate
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn set_sample_rate(&self, sr: f64) {
        *self
            .sample_rate
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = sr;
    }
}

// ---------------------------------------------------------------------------
// Editor ↔ host bridge
// ---------------------------------------------------------------------------

struct Vst3Bridge {
    gui: Arc<GuiState>,
}

// SAFETY: GUI-thread only; handler pointer is host-owned.
unsafe impl Send for Vst3Bridge {}
unsafe impl Sync for Vst3Bridge {}

impl Vst3Bridge {
    fn with_handler(&self, f: impl FnOnce(ComRef<'_, IComponentHandler>)) {
        let guard = self
            .gui
            .handler
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let Some(ptr) = *guard else {
            return;
        };
        // SAFETY: host set this via setComponentHandler; valid until cleared.
        if let Some(handler) = unsafe { ComRef::from_raw(ptr) } {
            f(handler);
        }
    }
}

impl EditorBridge for Vst3Bridge {
    fn begin_edit(&self, id: u32) {
        self.with_handler(|h| unsafe {
            let _ = h.beginEdit(id as ParamID);
        });
    }

    fn set_param(&self, id: u32, normalized: f64) {
        self.gui.params.set_normalized(id, normalized);
        self.with_handler(|h| unsafe {
            let _ = h.performEdit(id as ParamID, normalized as ParamValue);
        });
    }

    fn end_edit(&self, id: u32) {
        self.with_handler(|h| unsafe {
            let _ = h.endEdit(id as ParamID);
        });
    }

    fn get_param(&self, id: u32) -> f64 {
        self.gui.params.get_normalized(id).unwrap_or(0.0)
    }

    fn get_param_plain(&self, id: u32) -> f64 {
        self.gui.params.get_plain(id).unwrap_or(0.0)
    }

    fn request_resize(&self, _w: u32, _h: u32) -> bool {
        // IPlugFrame::resizeView needs the live IPlugView pointer; wire later
        // if a host requires dynamic resize from the plugin side.
        false
    }
}

// ---------------------------------------------------------------------------
// IPlugView
// ---------------------------------------------------------------------------

pub(crate) struct PlugView {
    gui: Arc<GuiState>,
}

impl PlugView {
    pub(crate) fn create(gui: Arc<GuiState>) -> *mut IPlugView {
        let wrapper = ComWrapper::new(Self { gui });
        match wrapper.to_com_ptr::<IPlugView>() {
            Some(ptr) => vst3::ComPtr::into_raw(ptr),
            None => std::ptr::null_mut(),
        }
    }
}

impl Class for PlugView {
    type Interfaces = (IPlugView,);
}

fn platform_type_bytes(ptr: FIDString) -> Option<&'static [u8]> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: FIDString is a host C string for platform type tags.
    Some(unsafe { CStr::from_ptr(ptr) }.to_bytes())
}

fn supported_platform(type_: FIDString) -> bool {
    let Some(bytes) = platform_type_bytes(type_) else {
        return false;
    };
    // Compare to Steinberg constants (NUL-terminated statics).
    #[cfg(target_os = "windows")]
    {
        let hwnd = unsafe { CStr::from_ptr(kPlatformTypeHWND) }.to_bytes();
        bytes == hwnd
    }
    #[cfg(target_os = "macos")]
    {
        let ns = unsafe { CStr::from_ptr(kPlatformTypeNSView) }.to_bytes();
        bytes == ns
    }
    #[cfg(target_os = "linux")]
    {
        let x11 = unsafe { CStr::from_ptr(kPlatformTypeX11EmbedWindowID) }.to_bytes();
        bytes == x11
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = bytes;
        false
    }
}

fn map_parent(parent: *mut c_void, type_: FIDString) -> Option<RawWindowHandle> {
    if parent.is_null() || !supported_platform(type_) {
        return None;
    }
    #[cfg(target_os = "windows")]
    {
        Some(RawWindowHandle::Win32(parent))
    }
    #[cfg(target_os = "macos")]
    {
        Some(RawWindowHandle::AppKit(parent))
    }
    #[cfg(target_os = "linux")]
    {
        // X11 window id passed as pointer-sized integer.
        Some(RawWindowHandle::X11(parent as u64))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = (parent, type_);
        None
    }
}

impl IPlugViewTrait for PlugView {
    unsafe fn isPlatformTypeSupported(&self, r#type: FIDString) -> tresult {
        if supported_platform(r#type) {
            kResultOk
        } else {
            kResultFalse
        }
    }

    unsafe fn attached(&self, parent: *mut c_void, r#type: FIDString) -> tresult {
        let Some(handle) = map_parent(parent, r#type) else {
            return kInvalidArgument;
        };

        let mut slot = self.gui.lock_editor();
        let Some(editor) = slot.as_mut() else {
            return kResultFalse;
        };

        let params = Arc::clone(&self.gui.params);
        let bridge = Arc::new(Vst3Bridge {
            gui: Arc::clone(&self.gui),
        });
        let ctx = PluginContext::new(params)
            .with_bridge(bridge)
            .with_sample_rate(self.gui.sample_rate());
        editor.open(handle, ctx);
        editor.show();
        kResultOk
    }

    unsafe fn removed(&self) -> tresult {
        if let Some(editor) = self.gui.lock_editor().as_mut() {
            editor.close();
        }
        kResultOk
    }

    unsafe fn onWheel(&self, _distance: f32) -> tresult {
        kResultFalse
    }

    unsafe fn onKeyDown(&self, _key: char16, _keyCode: int16, _modifiers: int16) -> tresult {
        kResultFalse
    }

    unsafe fn onKeyUp(&self, _key: char16, _keyCode: int16, _modifiers: int16) -> tresult {
        kResultFalse
    }

    unsafe fn getSize(&self, size: *mut ViewRect) -> tresult {
        if size.is_null() {
            return kInvalidArgument;
        }
        let (w, h) = self
            .gui
            .lock_editor()
            .as_ref()
            .map_or((320, 200), |e| e.size());
        // SAFETY: non-null host out-struct.
        unsafe {
            *size = ViewRect {
                left: 0,
                top: 0,
                right: w as int32,
                bottom: h as int32,
            };
        }
        kResultOk
    }

    unsafe fn onSize(&self, newSize: *mut ViewRect) -> tresult {
        if newSize.is_null() {
            return kInvalidArgument;
        }
        // SAFETY: non-null host struct for this call.
        let rect = unsafe { &*newSize };
        let w = (rect.right - rect.left).max(1) as u32;
        let h = (rect.bottom - rect.top).max(1) as u32;
        let mut slot = self.gui.lock_editor();
        let Some(editor) = slot.as_mut() else {
            return kResultFalse;
        };
        if editor.set_size(w, h) {
            kResultOk
        } else {
            kResultFalse
        }
    }

    unsafe fn onFocus(&self, _state: TBool) -> tresult {
        kResultOk
    }

    unsafe fn setFrame(&self, frame: *mut IPlugFrame) -> tresult {
        *self
            .gui
            .frame
            .lock()
            .unwrap_or_else(PoisonError::into_inner) =
            if frame.is_null() { None } else { Some(frame) };
        kResultOk
    }

    unsafe fn canResize(&self) -> tresult {
        let can = self
            .gui
            .lock_editor()
            .as_ref()
            .is_some_and(|e| e.can_resize());
        if can { kResultOk } else { kResultFalse }
    }

    unsafe fn checkSizeConstraint(&self, rect: *mut ViewRect) -> tresult {
        if rect.is_null() {
            return kInvalidArgument;
        }
        let slot = self.gui.lock_editor();
        let Some(editor) = slot.as_ref() else {
            return kResultOk;
        };
        let (min_w, min_h) = editor.min_size();
        let (max_w, max_h) = editor.max_size();
        // SAFETY: non-null host in/out rect.
        let r = unsafe { &mut *rect };
        let mut w = (r.right - r.left).max(1) as u32;
        let mut h = (r.bottom - r.top).max(1) as u32;
        w = w.clamp(min_w.max(1), max_w);
        h = h.clamp(min_h.max(1), max_h);
        r.right = r.left + w as int32;
        r.bottom = r.top + h as int32;
        kResultOk
    }
}
