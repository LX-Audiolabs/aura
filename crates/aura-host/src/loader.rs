//! CLAP plugin loader: dlopen .clap, factory, plugin lifecycle, param listing.

#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::missing_safety_doc)]

use std::ffi::{CStr, CString, c_char, c_void};
use std::ptr;
use clap_sys::{
    audio_buffer::clap_audio_buffer,
    entry::clap_plugin_entry,
    events::{clap_event_header, clap_input_events, clap_output_events},
    ext::params::{CLAP_EXT_PARAMS, CLAP_PARAM_IS_HIDDEN, clap_param_info, clap_plugin_params},
    factory::plugin_factory::{CLAP_PLUGIN_FACTORY_ID, clap_plugin_factory},
    host::clap_host,
    plugin::{clap_plugin, clap_plugin_descriptor},
    process::clap_process,
    version::CLAP_VERSION,
};

// ---------------------------------------------------------------------------
// Host callbacks — Phase 1: all no-ops, no extensions exposed
// ---------------------------------------------------------------------------

unsafe extern "C" fn host_get_extension(
    _: *const clap_host,
    _: *const c_char,
) -> *const c_void {
    ptr::null()
}
unsafe extern "C" fn host_request_restart(_: *const clap_host) {}
unsafe extern "C" fn host_request_process(_: *const clap_host) {}
unsafe extern "C" fn host_request_callback(_: *const clap_host) {}

/// Leak a `clap_host` with stable identity strings. Call once per process.
pub fn make_host() -> &'static clap_host {
    struct Strings {
        name: CString,
        vendor: CString,
        url: CString,
        version: CString,
    }
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
        let lib = unsafe { libloading::Library::new(path) }
            .map_err(|e| format!("dlopen: {e}"))?;

        let sym: libloading::Symbol<*const clap_plugin_entry> =
            unsafe { lib.get(b"clap_entry\0") }
                .map_err(|e| format!("clap_entry symbol: {e}"))?;
        let entry = unsafe { &**sym };

        let init = entry.init.ok_or("entry.init is null")?;
        let get_factory = entry.get_factory.ok_or("entry.get_factory is null")?;

        let path_c = CString::new(path).map_err(|e| e.to_string())?;
        if !unsafe { init(path_c.as_ptr()) } {
            return Err("entry.init returned false".into());
        }

        let factory_raw =
            unsafe { get_factory(CLAP_PLUGIN_FACTORY_ID.as_ptr()) };
        if factory_raw.is_null() {
            return Err("get_factory: no plugin factory".into());
        }

        Ok(Self {
            _lib: lib,
            factory: factory_raw as *const clap_plugin_factory,
            deinit: entry.deinit,
        })
    }

    pub fn plugin_count(&self) -> u32 {
        unsafe { &*self.factory }
            .get_plugin_count
            .map(|f| unsafe { f(self.factory) })
            .unwrap_or(0)
    }

    pub fn descriptor(&self, index: u32) -> Option<&clap_plugin_descriptor> {
        let f = unsafe { &*self.factory };
        f.get_plugin_descriptor.and_then(|g| {
            let p = unsafe { g(self.factory, index) };
            if p.is_null() { None } else { Some(unsafe { &*p }) }
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
            let Some(desc) = self.descriptor(i) else { continue };
            if desc.id.is_null() {
                continue;
            }
            let id_cstr = unsafe { CStr::from_ptr(desc.id) };
            if let Some(want) = want_id {
                if id_cstr.to_str().map_or(true, |s| s != want) {
                    continue;
                }
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

pub fn list_params(plugin: *const clap_plugin) {
    let get_ext = match unsafe { (*plugin).get_extension } {
        Some(f) => f,
        None => {
            println!("  (plugin has no get_extension)");
            return;
        }
    };
    let raw = unsafe { get_ext(plugin, CLAP_EXT_PARAMS.as_ptr()) };
    if raw.is_null() {
        println!("  (no clap.params extension)");
        return;
    }
    let params = unsafe { &*(raw as *const clap_plugin_params) };
    let count = params.count.map(|f| unsafe { f(plugin) }).unwrap_or(0);
    println!("{count} param(s):");
    let mut info: clap_param_info = unsafe { std::mem::zeroed() };
    for i in 0..count {
        let Some(get_info) = params.get_info else { break };
        if !unsafe { get_info(plugin, i, &mut info) } {
            continue;
        }
        if info.flags & CLAP_PARAM_IS_HIDDEN != 0 {
            continue;
        }
        let name = unsafe { CStr::from_ptr(info.name.as_ptr()) }.to_string_lossy();
        let val = params
            .get_value
            .and_then(|f| {
                let mut v = 0.0f64;
                if unsafe { f(plugin, info.id, &mut v) } { Some(v) } else { None }
            })
            .unwrap_or(f64::NAN);
        println!(
            "  [{i}] id={} {name} = {val:.4}  [{:.4}..{:.4}]",
            info.id, info.min_value, info.max_value
        );
    }
}

// ---------------------------------------------------------------------------
// Audio callback (cpal → CLAP process)
// ---------------------------------------------------------------------------

unsafe extern "C" fn ev_size_zero(_: *const clap_input_events) -> u32 {
    0
}
unsafe extern "C" fn ev_get_null(
    _: *const clap_input_events,
    _: u32,
) -> *const clap_event_header {
    ptr::null()
}
unsafe extern "C" fn ev_drop(
    _: *const clap_output_events,
    _: *const clap_event_header,
) -> bool {
    true
}

/// Newtype so `*const clap_plugin` can cross thread boundaries into the cpal callback.
#[derive(Copy, Clone)]
pub struct PluginPtr(pub *const clap_plugin);
unsafe impl Send for PluginPtr {}

/// Called from the cpal audio thread. `data` is interleaved f32 output.
pub fn audio_callback(pp: PluginPtr, data: &mut [f32], channels: usize) {
    let plugin = pp.0;
    if channels == 0 {
        return;
    }
    let frames = data.len() / channels;
    if frames == 0 {
        return;
    }

    // ponytail: heap-alloc per callback; replace with pre-alloc if profiling shows cost
    let mut left = vec![0f32; frames];
    let mut right = vec![0f32; frames];
    let mut out_ptrs: [*mut f32; 2] = [left.as_mut_ptr(), right.as_mut_ptr()];

    let mut audio_out = clap_audio_buffer {
        data32: out_ptrs.as_mut_ptr(),
        data64: ptr::null_mut(),
        channel_count: 2,
        latency: 0,
        constant_mask: 0,
    };

    // Stack-init event lists — process() is synchronous, refs stay valid.
    let in_ev = clap_input_events {
        ctx: ptr::null_mut(),
        size: Some(ev_size_zero),
        get: Some(ev_get_null),
    };
    let out_ev = clap_output_events {
        ctx: ptr::null_mut(),
        try_push: Some(ev_drop),
    };

    let proc = clap_process {
        steady_time: -1,
        frames_count: frames as u32,
        transport: ptr::null(),
        audio_inputs: ptr::null(),
        audio_outputs: &mut audio_out,
        audio_inputs_count: 0,
        audio_outputs_count: 1,
        in_events: &in_ev,
        out_events: &out_ev,
    };

    if let Some(process_fn) = unsafe { (*plugin).process } {
        unsafe { process_fn(plugin, &proc) };
    }

    // Interleave CLAP non-interleaved output into cpal interleaved buffer.
    for i in 0..frames {
        if channels >= 1 {
            data[i * channels] = left[i];
        }
        if channels >= 2 {
            data[i * channels + 1] = right[i];
        }
    }
}
