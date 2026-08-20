//! LV2 UI extension for AURA.
//!
//! Exposes the same [`Editor`] used by CLAP/VST3 as an LV2 embeddable UI.
//! The UI lives in the same shared library; `export_lv2!` also emits
//! `lv2ui_descriptor` so hosts can load it.

#![allow(clippy::missing_safety_doc)]
// ponytail: LV2 FFI — raw pointers, platform casts, spec-shaped.
#![allow(
    clippy::ptr_as_ptr,
    clippy::ref_as_ptr,
    clippy::borrow_as_ptr,
    clippy::cast_ptr_alignment,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::similar_names,
    clippy::must_use_candidate,
    clippy::items_after_statements,
    clippy::uninlined_format_args
)]

use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_void};
use std::ptr;
use std::sync::{Arc, OnceLock};

use aura_core::{
    PluginLogic,
    editor::{Editor, EditorBridge, PluginContext, RawWindowHandle},
    host_callback, host_callback_with,
};
use aura_params::Params;
use lv2_sys::{
    LV2_Feature, LV2_UI__idleInterface, LV2_UI__parent, LV2_UI__resize, LV2UI_Controller,
    LV2UI_Descriptor, LV2UI_Handle, LV2UI_Idle_Interface, LV2UI_Resize, LV2UI_Widget,
    LV2UI_Write_Function,
};

use crate::{param_list, plugin_uri, uri_cstring};

// ---------------------------------------------------------------------------
// URI / descriptor
// ---------------------------------------------------------------------------

fn ui_uri_cstring<L: PluginLogic>() -> &'static CStr {
    static CELL: OnceLock<CString> = OnceLock::new();
    CELL.get_or_init(|| {
        let s = format!("{}#ui", plugin_uri(&L::info()));
        CString::new(s).expect("UI URI has interior NUL")
    })
    .as_c_str()
}

/// LV2 UI descriptor — returned by `lv2ui_descriptor`.
#[must_use]
pub fn ui_descriptor<L: PluginLogic>(index: u32) -> *const LV2UI_Descriptor {
    if index != 0 {
        return ptr::null();
    }
    // If the plugin declares no editor, expose no UI.
    let has_editor = L::editor(Arc::new(L::Params::default())).is_some();
    if !has_editor {
        return ptr::null();
    }
    static CELL: OnceLock<usize> = OnceLock::new();
    let addr = *CELL.get_or_init(|| {
        let desc = Box::new(LV2UI_Descriptor {
            URI: ui_uri_cstring::<L>().as_ptr(),
            instantiate: Some(ui_instantiate::<L>),
            cleanup: Some(ui_cleanup::<L>),
            port_event: Some(ui_port_event::<L>),
            extension_data: Some(ui_extension_data::<L>),
        });
        Box::into_raw(desc) as usize
    });
    addr as *const LV2UI_Descriptor
}

// ---------------------------------------------------------------------------
// Instance
// ---------------------------------------------------------------------------

struct UiInstance<L: PluginLogic> {
    params: Arc<L::Params>,
    editor: Box<dyn Editor>,
    /// X11 needs the widget pointer to stay valid, so keep the Window id here.
    #[cfg(target_os = "linux")]
    x11_window: u64,
}

impl<L: PluginLogic> UiInstance<L> {
    fn from_handle<'a>(handle: LV2UI_Handle) -> Option<&'a mut Self> {
        if handle.is_null() {
            return None;
        }
        Some(unsafe { &mut *(handle as *mut Self) })
    }
}

/// Map a param id to the LV2 control-port index.
fn build_port_map<L: PluginLogic>() -> HashMap<u32, u32> {
    let infos = param_list::<L>();
    let ctrl0 = crate::audio_port_count(crate::static_layout::<L>()) as u32;
    infos
        .iter()
        .enumerate()
        .map(|(i, info)| (info.id, ctrl0 + i as u32))
        .collect()
}

// ---------------------------------------------------------------------------
// Host bridge
// ---------------------------------------------------------------------------

struct Lv2Bridge {
    params: Arc<dyn Params>,
    write: LV2UI_Write_Function,
    controller: LV2UI_Controller,
    port_by_id: HashMap<u32, u32>,
    resize: Option<LV2UI_Resize>,
}

// SAFETY: function pointers are host-owned and only used on the UI thread.
unsafe impl Send for Lv2Bridge {}
unsafe impl Sync for Lv2Bridge {}

impl Lv2Bridge {
    fn write_control(&self, port_index: u32, value: f32) {
        let Some(write) = self.write else {
            return;
        };
        unsafe {
            write(
                self.controller,
                port_index,
                size_of::<f32>() as u32,
                0,
                (&value as *const f32).cast(),
            );
        }
    }
}

impl EditorBridge for Lv2Bridge {
    fn begin_edit(&self, _id: u32) {
        // LV2 has ui:touch, but most hosts do not require it for simple params.
    }

    fn set_param(&self, id: u32, normalized: f64) {
        self.params.set_normalized(id, normalized);
        let Some(port_index) = self.port_by_id.get(&id).copied() else {
            return;
        };
        let plain = self.params.get_plain(id).unwrap_or(0.0);
        self.write_control(port_index, plain as f32);
    }

    fn end_edit(&self, _id: u32) {}

    fn get_param(&self, id: u32) -> f64 {
        self.params.get_normalized(id).unwrap_or(0.0)
    }

    fn get_param_plain(&self, id: u32) -> f64 {
        self.params.get_plain(id).unwrap_or(0.0)
    }

    fn request_resize(&self, w: u32, h: u32) -> bool {
        let Some(resize) = self.resize else {
            return false;
        };
        let Some(ui_resize) = resize.ui_resize else {
            return false;
        };
        unsafe {
            ui_resize(
                resize.handle,
                w as std::os::raw::c_int,
                h as std::os::raw::c_int,
            ) == 0
        }
    }
}

// ---------------------------------------------------------------------------
// Instantiate / cleanup
// ---------------------------------------------------------------------------

unsafe extern "C" fn ui_instantiate<L: PluginLogic>(
    _descriptor: *const LV2UI_Descriptor,
    plugin_uri: *const c_char,
    _bundle_path: *const c_char,
    write_function: LV2UI_Write_Function,
    controller: LV2UI_Controller,
    widget: *mut LV2UI_Widget,
    features: *const *const LV2_Feature,
) -> LV2UI_Handle {
    host_callback_with("LV2 UI", "instantiate", ptr::null_mut(), || {
        if plugin_uri.is_null() || widget.is_null() {
            return ptr::null_mut();
        }
        let want = unsafe { CStr::from_ptr(plugin_uri) };
        if want.to_bytes() != uri_cstring::<L>().to_bytes() {
            return ptr::null_mut();
        }

        let mut parent_widget: Option<LV2UI_Widget> = None;
        let mut resize: Option<LV2UI_Resize> = None;

        if !features.is_null() {
            let mut i = 0isize;
            loop {
                let f = unsafe { *features.offset(i) };
                if f.is_null() {
                    break;
                }
                let uri = unsafe { (*f).URI };
                if !uri.is_null() {
                    let u = unsafe { CStr::from_ptr(uri) };
                    if u.to_bytes() == &LV2_UI__parent[..LV2_UI__parent.len() - 1] {
                        // data points to the parent widget value (LV2UI_Widget*).
                        let data = unsafe { (*f).data };
                        if !data.is_null() {
                            parent_widget = Some(unsafe { *(data as *mut LV2UI_Widget) });
                        }
                    } else if u.to_bytes() == &LV2_UI__resize[..LV2_UI__resize.len() - 1] {
                        let data = unsafe { (*f).data };
                        if !data.is_null() {
                            resize = Some(unsafe { *(data as *mut LV2UI_Resize) });
                        }
                    }
                }
                i += 1;
            }
        }

        let Some(parent_widget) = parent_widget else {
            return ptr::null_mut();
        };
        let Some(parent_handle) = map_parent_widget(parent_widget) else {
            return ptr::null_mut();
        };

        let params = Arc::new(L::Params::default());
        let Some(mut editor) = L::editor(Arc::clone(&params)) else {
            return ptr::null_mut();
        };

        let port_by_id = build_port_map::<L>();
        let bridge = Arc::new(Lv2Bridge {
            params: Arc::clone(&params) as Arc<dyn Params>,
            write: write_function,
            controller,
            port_by_id,
            resize,
        });
        let ctx =
            PluginContext::new(Arc::clone(&params) as Arc<dyn Params>).with_bridge(bridge.clone());

        editor.open(parent_handle, ctx);
        editor.show();

        #[cfg(target_os = "linux")]
        let x11_window = match editor.native_handle() {
            Some(RawWindowHandle::X11(id)) => id,
            _ => 0,
        };

        let _bridge = bridge;
        let inst = Box::new(UiInstance::<L> {
            params,
            editor,
            #[cfg(target_os = "linux")]
            x11_window,
        });
        let ptr = Box::into_raw(inst);

        // Write the widget pointer back to the host.
        unsafe {
            #[cfg(target_os = "windows")]
            {
                *widget = match (*ptr).editor.native_handle() {
                    Some(RawWindowHandle::Win32(hwnd)) => hwnd,
                    _ => parent_widget,
                };
            }
            #[cfg(target_os = "macos")]
            {
                *widget = match (*ptr).editor.native_handle() {
                    Some(RawWindowHandle::AppKit(view)) => view,
                    _ => parent_widget,
                };
            }
            #[cfg(target_os = "linux")]
            {
                *widget = &raw mut (*ptr).x11_window as *mut u64 as LV2UI_Widget;
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
            {
                *widget = parent_widget;
            }
        }

        ptr as LV2UI_Handle
    })
}

unsafe extern "C" fn ui_cleanup<L: PluginLogic>(ui: LV2UI_Handle) {
    if ui.is_null() {
        return;
    }
    host_callback("LV2 UI", "cleanup", || {
        let inst = unsafe { Box::from_raw(ui as *mut UiInstance<L>) };
        let _ = inst;
    });
}

// ---------------------------------------------------------------------------
// Port events
// ---------------------------------------------------------------------------

unsafe extern "C" fn ui_port_event<L: PluginLogic>(
    ui: LV2UI_Handle,
    port_index: u32,
    buffer_size: u32,
    format: u32,
    buffer: *const c_void,
) {
    host_callback("LV2 UI", "port_event", || {
        let Some(inst) = UiInstance::<L>::from_handle(ui) else {
            return;
        };
        // format 0 = lv2:ControlPort, buffer is a single float.
        if format != 0 || buffer_size < size_of::<f32>() as u32 || buffer.is_null() {
            return;
        }
        let value = f64::from(unsafe { *(buffer as *const f32) });

        let infos = param_list::<L>();
        let ctrl0 = crate::audio_port_count(crate::static_layout::<L>()) as u32;
        let idx = port_index.saturating_sub(ctrl0) as usize;
        if let Some(info) = infos.get(idx) {
            inst.params.set_plain(info.id, value);
            inst.editor.state_changed();
        }
    });
}

// ---------------------------------------------------------------------------
// Extension data (idle interface)
// ---------------------------------------------------------------------------

unsafe extern "C" fn ui_extension_data<L: PluginLogic>(uri: *const c_char) -> *const c_void {
    if uri.is_null() {
        return ptr::null();
    }
    let u = unsafe { CStr::from_ptr(uri) };
    if u.to_bytes() == &LV2_UI__idleInterface[..LV2_UI__idleInterface.len() - 1] {
        return idle_iface::<L>() as *const _ as *const c_void;
    }
    ptr::null()
}

fn idle_iface<L: PluginLogic>() -> &'static LV2UI_Idle_Interface {
    static CELL: OnceLock<LV2UI_Idle_Interface> = OnceLock::new();
    CELL.get_or_init(|| LV2UI_Idle_Interface {
        idle: Some(ui_idle::<L>),
    })
}

unsafe extern "C" fn ui_idle<L: PluginLogic>(ui: LV2UI_Handle) -> std::os::raw::c_int {
    host_callback_with("LV2 UI", "idle", 0, || {
        let Some(inst) = UiInstance::<L>::from_handle(ui) else {
            return 1;
        };
        inst.editor.idle();
        0
    })
}

// ---------------------------------------------------------------------------
// Platform mapping
// ---------------------------------------------------------------------------

fn map_parent_widget(widget: LV2UI_Widget) -> Option<RawWindowHandle> {
    if widget.is_null() {
        return None;
    }
    #[cfg(target_os = "windows")]
    {
        Some(RawWindowHandle::Win32(widget))
    }
    #[cfg(target_os = "macos")]
    {
        Some(RawWindowHandle::AppKit(widget))
    }
    #[cfg(target_os = "linux")]
    {
        // LV2 X11UI widget is a Window (unsigned long); the feature data is Window*.
        Some(RawWindowHandle::X11(widget as u64))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CStr;
    use std::sync::Arc;

    use aura_core::{
        AudioBuffer, AudioConfig, PluginLogic, ProcessContext, ProcessStatus,
        editor::{Editor, PluginContext, RawWindowHandle},
        info::PluginInfo,
    };
    use aura_params::FloatParam;

    use super::{ui_descriptor, ui_extension_data};

    #[derive(aura_derive::Params)]
    struct TestParams {
        #[param(id = 0, name = "Gain")]
        gain: FloatParam,
    }

    #[derive(Default)]
    struct DummyEditor {
        handle: Option<RawWindowHandle>,
    }

    impl Editor for DummyEditor {
        fn size(&self) -> (u32, u32) {
            (100, 100)
        }

        fn open(&mut self, parent: RawWindowHandle, _context: PluginContext) {
            self.handle = Some(parent);
        }

        fn close(&mut self) {
            self.handle = None;
        }

        fn native_handle(&self) -> Option<RawWindowHandle> {
            self.handle
        }
    }

    struct TestLogic;

    impl PluginLogic for TestLogic {
        type Params = TestParams;
        type DspState = ();

        fn info() -> PluginInfo {
            PluginInfo::new("Test", "LX", "0.1.0", "test-ui")
        }

        fn init(_params: &Self::Params, _sample_rate: f64) -> Self::DspState {}

        fn reset(_state: &mut Self::DspState, _params: &Self::Params, _config: &AudioConfig) {}

        fn process(
            _state: &mut Self::DspState,
            _params: &Self::Params,
            _buffer: &mut AudioBuffer<'_, f32>,
            _context: &mut ProcessContext,
        ) -> ProcessStatus {
            ProcessStatus::Continue
        }

        fn editor(_params: Arc<Self::Params>) -> Option<Box<dyn Editor>> {
            Some(Box::<DummyEditor>::default())
        }
    }

    struct NoEditorLogic;

    impl PluginLogic for NoEditorLogic {
        type Params = TestParams;
        type DspState = ();

        fn info() -> PluginInfo {
            PluginInfo::new("Test", "LX", "0.1.0", "test-no-ui")
        }

        fn init(_params: &Self::Params, _sample_rate: f64) -> Self::DspState {}

        fn reset(_state: &mut Self::DspState, _params: &Self::Params, _config: &AudioConfig) {}

        fn process(
            _state: &mut Self::DspState,
            _params: &Self::Params,
            _buffer: &mut AudioBuffer<'_, f32>,
            _context: &mut ProcessContext,
        ) -> ProcessStatus {
            ProcessStatus::Continue
        }
    }

    #[test]
    fn ui_descriptor_returns_single_entry() {
        let desc = ui_descriptor::<TestLogic>(0);
        assert!(!desc.is_null());
        assert!(ui_descriptor::<TestLogic>(1).is_null());
    }

    #[test]
    fn ui_descriptor_null_when_plugin_has_no_editor() {
        assert!(ui_descriptor::<NoEditorLogic>(0).is_null());
    }

    #[test]
    fn ui_extension_data_exposes_idle_interface() {
        let uri = CStr::from_bytes_with_nul(super::LV2_UI__idleInterface).unwrap();
        // SAFETY: test-only call with a valid LV2 feature URI.
        assert!(!unsafe { ui_extension_data::<TestLogic>(uri.as_ptr()) }.is_null());
    }

    #[test]
    fn ui_extension_data_null_for_unknown_uri() {
        let uri = c"http://example.com/unknown";
        // SAFETY: test-only calls with known-safe pointer inputs.
        assert!(unsafe { ui_extension_data::<TestLogic>(uri.as_ptr()) }.is_null());
        assert!(unsafe { ui_extension_data::<TestLogic>(std::ptr::null()) }.is_null());
    }
}
