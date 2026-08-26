//! CLAP plugin loader: dlopen .clap, factory, plugin lifecycle, param listing.

#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::missing_safety_doc)]

use clap_sys::{
    entry::clap_plugin_entry,
    ext::{
        audio_ports::{CLAP_EXT_AUDIO_PORTS, clap_audio_port_info, clap_plugin_audio_ports},
        gui::{CLAP_EXT_GUI, clap_host_gui},
        log::{
            CLAP_EXT_LOG, CLAP_LOG_ERROR, CLAP_LOG_FATAL, CLAP_LOG_HOST_MISBEHAVING, CLAP_LOG_INFO,
            CLAP_LOG_PLUGIN_MISBEHAVING, CLAP_LOG_WARNING, clap_host_log, clap_log_severity,
        },
        note_ports::{
            CLAP_EXT_NOTE_PORTS, CLAP_NOTE_DIALECT_CLAP, CLAP_NOTE_DIALECT_MIDI,
            clap_note_port_info, clap_plugin_note_ports,
        },
        params::{
            CLAP_EXT_PARAMS, CLAP_PARAM_IS_HIDDEN, clap_host_params, clap_param_clear_flags,
            clap_param_info, clap_param_rescan_flags, clap_plugin_params,
        },
        thread_check::{CLAP_EXT_THREAD_CHECK, clap_host_thread_check},
    },
    factory::plugin_factory::{CLAP_PLUGIN_FACTORY_ID, clap_plugin_factory},
    host::clap_host,
    id::clap_id,
    plugin::{clap_plugin, clap_plugin_descriptor},
    version::CLAP_VERSION,
};
use std::ffi::{CStr, CString, c_char, c_void};
use std::ptr;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::ThreadId;

use crate::events::{Dialect, EvList, sink_output_events};

// ---------------------------------------------------------------------------
// Host extensions — log + thread_check (params/gui land in Phase 2)
// ---------------------------------------------------------------------------

static MAIN_THREAD: OnceLock<ThreadId> = OnceLock::new();
static AUDIO_THREAD: OnceLock<ThreadId> = OnceLock::new();

/// Called once from the cpal callback so `is_audio_thread` can answer honestly.
pub fn mark_audio_thread() {
    let _ = AUDIO_THREAD.set(std::thread::current().id());
}

unsafe extern "C" fn host_log(
    _: *const clap_host,
    severity: clap_log_severity,
    msg: *const c_char,
) {
    let text = if msg.is_null() {
        std::borrow::Cow::Borrowed("(null)")
    } else {
        unsafe { CStr::from_ptr(msg) }.to_string_lossy()
    };
    let tag = match severity {
        CLAP_LOG_INFO => "info",
        CLAP_LOG_WARNING => "warn",
        CLAP_LOG_ERROR => "error",
        CLAP_LOG_FATAL => "fatal",
        CLAP_LOG_HOST_MISBEHAVING => "host-misbehaving",
        CLAP_LOG_PLUGIN_MISBEHAVING => "plugin-misbehaving",
        _ => "debug",
    };
    eprintln!("[plugin/{tag}] {text}");
}

unsafe extern "C" fn host_is_main_thread(_: *const clap_host) -> bool {
    MAIN_THREAD.get() == Some(&std::thread::current().id())
}

unsafe extern "C" fn host_is_audio_thread(_: *const clap_host) -> bool {
    AUDIO_THREAD.get() == Some(&std::thread::current().id())
}

/// Set by `request_callback`; drained on the main thread by [`pump_main_thread`].
static CALLBACK_REQUESTED: AtomicBool = AtomicBool::new(false);
/// Set by `request_restart`; the GUI reopens the audio session when it sees it.
static RESTART_REQUESTED: AtomicBool = AtomicBool::new(false);
/// Set by the plugin's `gui.closed` callback — the window is gone.
static GUI_CLOSED: AtomicBool = AtomicBool::new(false);

/// Run the plugin's pending main-thread work. Call from the UI event loop.
pub fn pump_main_thread(plugin: *const clap_plugin) {
    if CALLBACK_REQUESTED.swap(false, Ordering::AcqRel)
        && let Some(cb) = unsafe { (*plugin).on_main_thread }
    {
        unsafe { cb(plugin) };
    }
}

/// True once per `request_restart` from the plugin.
pub fn take_restart_request() -> bool {
    RESTART_REQUESTED.swap(false, Ordering::AcqRel)
}

/// True once per `gui.closed` from the plugin.
pub fn take_gui_closed() -> bool {
    GUI_CLOSED.swap(false, Ordering::AcqRel)
}

unsafe extern "C" fn host_gui_resize_hints_changed(_: *const clap_host) {}
unsafe extern "C" fn host_gui_request_resize(_: *const clap_host, _: u32, _: u32) -> bool {
    // Floating windows size themselves; nothing for us to resize.
    false
}
unsafe extern "C" fn host_gui_request_show(_: *const clap_host) -> bool {
    false
}
unsafe extern "C" fn host_gui_request_hide(_: *const clap_host) -> bool {
    false
}
unsafe extern "C" fn host_gui_closed(_: *const clap_host, _was_destroyed: bool) {
    GUI_CLOSED.store(true, Ordering::Release);
}

unsafe extern "C" fn host_params_rescan(_: *const clap_host, _: clap_param_rescan_flags) {}
unsafe extern "C" fn host_params_clear(_: *const clap_host, _: clap_id, _: clap_param_clear_flags) {
}
unsafe extern "C" fn host_params_request_flush(_: *const clap_host) {}

static LOG_EXT: clap_host_log = clap_host_log {
    log: Some(host_log),
};
static GUI_EXT: clap_host_gui = clap_host_gui {
    resize_hints_changed: Some(host_gui_resize_hints_changed),
    request_resize: Some(host_gui_request_resize),
    request_show: Some(host_gui_request_show),
    request_hide: Some(host_gui_request_hide),
    closed: Some(host_gui_closed),
};
// The UI polls `params.get_value` instead of tracking output events, so these
// are accept-and-ignore. Plugins still expect the extension to exist.
static PARAMS_EXT: clap_host_params = clap_host_params {
    rescan: Some(host_params_rescan),
    clear: Some(host_params_clear),
    request_flush: Some(host_params_request_flush),
};
static THREAD_CHECK_EXT: clap_host_thread_check = clap_host_thread_check {
    is_main_thread: Some(host_is_main_thread),
    is_audio_thread: Some(host_is_audio_thread),
};

unsafe extern "C" fn host_get_extension(_: *const clap_host, id: *const c_char) -> *const c_void {
    if id.is_null() {
        return ptr::null();
    }
    let id = unsafe { CStr::from_ptr(id) };
    if id == CLAP_EXT_LOG {
        return ptr::from_ref(&LOG_EXT).cast();
    }
    if id == CLAP_EXT_THREAD_CHECK {
        return ptr::from_ref(&THREAD_CHECK_EXT).cast();
    }
    if id == CLAP_EXT_GUI {
        return ptr::from_ref(&GUI_EXT).cast();
    }
    if id == CLAP_EXT_PARAMS {
        return ptr::from_ref(&PARAMS_EXT).cast();
    }
    ptr::null()
}
unsafe extern "C" fn host_request_restart(_: *const clap_host) {
    RESTART_REQUESTED.store(true, Ordering::Release);
}
unsafe extern "C" fn host_request_process(_: *const clap_host) {}
unsafe extern "C" fn host_request_callback(_: *const clap_host) {
    CALLBACK_REQUESTED.store(true, Ordering::Release);
}

/// Leak a `clap_host` with stable identity strings. Call once, from the main thread.
pub fn make_host() -> &'static clap_host {
    struct Strings {
        name: CString,
        vendor: CString,
        url: CString,
        version: CString,
    }
    let _ = MAIN_THREAD.set(std::thread::current().id());
    let s = Box::leak(Box::new(Strings {
        name: CString::new("aura-host").unwrap(),
        vendor: CString::new("LX Audiolabs").unwrap(),
        url: CString::new("https://github.com/LX-Audiolabs/aura").unwrap(),
        version: CString::new(env!("CARGO_PKG_VERSION")).unwrap(),
    }));
    Box::leak(Box::new(clap_host {
        clap_version: CLAP_VERSION,
        host_data: ptr::null_mut(),
        name: s.name.as_ptr(),
        vendor: s.vendor.as_ptr(),
        url: s.url.as_ptr(),
        version: s.version.as_ptr(),
        get_extension: Some(host_get_extension),
        request_restart: Some(host_request_restart),
        request_process: Some(host_request_process),
        request_callback: Some(host_request_callback),
    }))
}

// ---------------------------------------------------------------------------
// Loader
// ---------------------------------------------------------------------------

pub struct Loader {
    _lib: libloading::Library,
    factory: *const clap_plugin_factory,
    deinit: Option<unsafe extern "C" fn()>,
}

// Safety: we never share the Loader across threads.
unsafe impl Send for Loader {}

impl Loader {
    /// dlopen `path`, call `clap_entry.init`, and locate the plugin factory.
    pub unsafe fn open(path: &str) -> Result<Self, String> {
        let lib = unsafe { libloading::Library::new(path) }.map_err(|e| format!("dlopen: {e}"))?;

        let sym: libloading::Symbol<*const clap_plugin_entry> =
            unsafe { lib.get(b"clap_entry\0") }.map_err(|e| format!("clap_entry symbol: {e}"))?;
        let entry = unsafe { &**sym };

        let init = entry.init.ok_or("entry.init is null")?;
        let get_factory = entry.get_factory.ok_or("entry.get_factory is null")?;

        let path_c = CString::new(path).map_err(|e| e.to_string())?;
        if !unsafe { init(path_c.as_ptr()) } {
            return Err("entry.init returned false".into());
        }

        let factory_raw = unsafe { get_factory(CLAP_PLUGIN_FACTORY_ID.as_ptr()) };
        if factory_raw.is_null() {
            return Err("get_factory: no plugin factory".into());
        }

        Ok(Self {
            _lib: lib,
            factory: factory_raw.cast::<clap_plugin_factory>(),
            deinit: entry.deinit,
        })
    }

    pub fn plugin_count(&self) -> u32 {
        unsafe { &*self.factory }
            .get_plugin_count
            .map_or(0, |f| unsafe { f(self.factory) })
    }

    pub fn descriptor(&self, index: u32) -> Option<&clap_plugin_descriptor> {
        let f = unsafe { &*self.factory };
        f.get_plugin_descriptor.and_then(|g| {
            let p = unsafe { g(self.factory, index) };
            if p.is_null() {
                None
            } else {
                Some(unsafe { &*p })
            }
        })
    }

    /// Create the first plugin whose id matches `want_id` (or the first plugin
    /// if `want_id` is `None`).
    pub fn create(
        &self,
        host: *const clap_host,
        want_id: Option<&str>,
    ) -> Result<*const clap_plugin, String> {
        let f = unsafe { &*self.factory };
        let create_fn = f.create_plugin.ok_or("factory.create_plugin is null")?;
        for i in 0..self.plugin_count() {
            let Some(desc) = self.descriptor(i) else {
                continue;
            };
            if desc.id.is_null() {
                continue;
            }
            let id_cstr = unsafe { CStr::from_ptr(desc.id) };
            if let Some(want) = want_id
                && id_cstr.to_str() != Ok(want)
            {
                continue;
            }
            let p = unsafe { create_fn(self.factory, host, desc.id) };
            if !p.is_null() {
                return Ok(p);
            }
        }
        Err(match want_id {
            Some(id) => format!("plugin id={id:?} not found"),
            None => "factory.create_plugin returned null for all plugins".into(),
        })
    }
}

impl Drop for Loader {
    fn drop(&mut self) {
        if let Some(deinit) = self.deinit {
            unsafe { deinit() };
        }
    }
}

// ---------------------------------------------------------------------------
// Param listing
// ---------------------------------------------------------------------------

/// Fetch a plugin extension by id. Returns `None` if unsupported.
pub fn plugin_ext(plugin: *const clap_plugin, id: &CStr) -> Option<*const c_void> {
    let get_ext = unsafe { (*plugin).get_extension }?;
    let raw = unsafe { get_ext(plugin, id.as_ptr()) };
    if raw.is_null() { None } else { Some(raw) }
}

fn params_ext(plugin: *const clap_plugin) -> Option<&'static clap_plugin_params> {
    plugin_ext(plugin, CLAP_EXT_PARAMS).map(|p| unsafe { &*p.cast::<clap_plugin_params>() })
}

/// Channel count of every audio port on one side (`is_input` picks the side).
/// The host must hand `process()` a buffer for *each* declared port, so this
/// returns all of them, not just the main one.
pub fn audio_port_channels(plugin: *const clap_plugin, is_input: bool) -> Vec<u32> {
    let Some(ext) = plugin_ext(plugin, CLAP_EXT_AUDIO_PORTS) else {
        return Vec::new();
    };
    let ports = unsafe { &*ext.cast::<clap_plugin_audio_ports>() };
    let count = ports.count.map_or(0, |f| unsafe { f(plugin, is_input) });
    let Some(get) = ports.get else {
        return Vec::new();
    };
    let mut info: clap_audio_port_info = unsafe { std::mem::zeroed() };
    (0..count)
        .map(|i| {
            if unsafe { get(plugin, i, is_input, &raw mut info) } {
                info.channel_count
            } else {
                0
            }
        })
        .collect()
}

/// Note dialect the plugin's first *input* note port prefers.
pub fn note_dialect(plugin: *const clap_plugin) -> Dialect {
    let Some(ext) = plugin_ext(plugin, CLAP_EXT_NOTE_PORTS) else {
        return Dialect::None;
    };
    let ports = unsafe { &*ext.cast::<clap_plugin_note_ports>() };
    if ports.count.map_or(0, |f| unsafe { f(plugin, true) }) == 0 {
        return Dialect::None;
    }
    let Some(get) = ports.get else {
        return Dialect::None;
    };
    let mut info: clap_note_port_info = unsafe { std::mem::zeroed() };
    if !unsafe { get(plugin, 0, true, &raw mut info) } {
        return Dialect::None;
    }
    // Prefer what the port asks for; fall back to anything it supports.
    for d in [info.preferred_dialect, info.supported_dialects] {
        if d & CLAP_NOTE_DIALECT_MIDI != 0 {
            return Dialect::Midi;
        }
        if d & CLAP_NOTE_DIALECT_CLAP != 0 {
            return Dialect::Clap;
        }
    }
    Dialect::None
}

/// Set one param on the *deactivated* plugin via `params.flush` (main thread).
pub fn set_param(plugin: *const clap_plugin, id: clap_id, value: f64) -> Result<(), String> {
    let params = params_ext(plugin).ok_or("plugin has no clap.params extension")?;
    let flush = params.flush.ok_or("clap.params has no flush")?;
    let mut evs = EvList::with_capacity(1);
    evs.push_param(id, value, 0);
    let in_ev = evs.as_input_events();
    let out_ev = sink_output_events();
    unsafe { flush(plugin, &raw const in_ev, &raw const out_ev) };
    Ok(())
}

/// One visible parameter, as the UI and the CLI listing both want it.
#[derive(Clone, Debug)]
pub struct ParamInfo {
    pub id: clap_id,
    pub name: String,
    pub min: f64,
    pub max: f64,
    pub value: f64,
}

/// Every non-hidden parameter, with its current value. Main thread only.
#[must_use]
pub fn params(plugin: *const clap_plugin) -> Vec<ParamInfo> {
    let Some(params) = params_ext(plugin) else {
        return Vec::new();
    };
    let (Some(get_info), count) = (
        params.get_info,
        params.count.map_or(0, |f| unsafe { f(plugin) }),
    ) else {
        return Vec::new();
    };
    let mut info: clap_param_info = unsafe { std::mem::zeroed() };
    (0..count)
        .filter_map(|i| {
            if !unsafe { get_info(plugin, i, &raw mut info) }
                || info.flags & CLAP_PARAM_IS_HIDDEN != 0
            {
                return None;
            }
            Some(ParamInfo {
                id: info.id,
                name: unsafe { CStr::from_ptr(info.name.as_ptr()) }
                    .to_string_lossy()
                    .into_owned(),
                min: info.min_value,
                max: info.max_value,
                value: param_value(plugin, info.id).unwrap_or(f64::NAN),
            })
        })
        .collect()
}

/// Current value of one parameter. Main thread only.
#[must_use]
pub fn param_value(plugin: *const clap_plugin, id: clap_id) -> Option<f64> {
    let get = params_ext(plugin)?.get_value?;
    let mut v = 0.0f64;
    unsafe { get(plugin, id, &raw mut v) }.then_some(v)
}

/// The plugin's own formatting for a value ("-6.0 dB"). Falls back to the
/// plain number when the plugin has no `value_to_text`.
#[must_use]
pub fn param_text(plugin: *const clap_plugin, id: clap_id, value: f64) -> String {
    let fallback = || format!("{value:.2}");
    let Some(to_text) = params_ext(plugin).and_then(|p| p.value_to_text) else {
        return fallback();
    };
    let mut buf = [0u8; 64];
    if !unsafe {
        to_text(
            plugin,
            id,
            value,
            buf.as_mut_ptr().cast::<c_char>(),
            buf.len() as u32,
        )
    } {
        return fallback();
    }
    CStr::from_bytes_until_nul(&buf)
        .map_or_else(|_| fallback(), |s| s.to_string_lossy().into_owned())
}

pub fn list_params(plugin: *const clap_plugin) {
    let all = params(plugin);
    if all.is_empty() {
        println!("  (no params)");
        return;
    }
    println!("{} param(s):", all.len());
    for (i, p) in all.iter().enumerate() {
        println!(
            "  [{i}] id={} {} = {:.4} ({})  [{:.4}..{:.4}]",
            p.id,
            p.name,
            p.value,
            param_text(plugin, p.id, p.value),
            p.min,
            p.max
        );
    }
}

/// Newtype so `*const clap_plugin` can cross thread boundaries into the cpal callback.
#[derive(Copy, Clone)]
pub struct PluginPtr(pub *const clap_plugin);
unsafe impl Send for PluginPtr {}
