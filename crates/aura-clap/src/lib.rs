//! Minimal CLAP wrapper for AURA.
//!
//! **Spec:** [free-audio/clap](https://github.com/free-audio/clap) is the source of truth.
//! Bindings: [`clap-sys`](https://crates.io/crates/clap-sys) (may lag free-audio *revision*;
//! see crate README). Extensions and layouts always follow free-audio headers.
//!
//! ```ignore
//! aura_clap::export!(MyPlugin);
//! ```
//!
//! Covers: `clap_entry` factory, audio ports (mono/stereo layouts), params,
//! process, state, GUI, remote-controls, latency, optional audio-ports-config.

#![allow(clippy::missing_safety_doc)]
// ponytail: CLAP FFI glue — raw-pointer casts and C-int size conversions are
// spec-shaped; the "safer" spellings add noise without changing semantics.
#![allow(
    clippy::ptr_as_ptr,
    clippy::ref_as_ptr,
    clippy::borrow_as_ptr,
    clippy::cast_ptr_alignment,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

use std::ffi::{CStr, CString, c_char, c_void};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};

use aura_core::editor::{Editor, EditorBridge, PluginContext, RawWindowHandle};
use aura_core::events::{ParamEvent, ParamEventQueue};
use aura_core::info::PluginCategory;
use aura_core::transport::Transport;
use aura_core::{
    AudioBuffer, AudioConfig, BusLayout, ChannelConfig, PluginLogic, ProcessContext, ProcessMode,
    ProcessStatus, host_callback, host_callback_with, layout_at,
};
use aura_core::{MidiBuffer, MidiMessage};
use aura_params::{ParamFlags, ParamInfo, ParamRange, Params};
use clap_sys::events::{
    CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_IS_LIVE, CLAP_EVENT_MIDI, CLAP_EVENT_NOTE_CHOKE,
    CLAP_EVENT_NOTE_OFF, CLAP_EVENT_NOTE_ON, CLAP_EVENT_PARAM_GESTURE_BEGIN,
    CLAP_EVENT_PARAM_GESTURE_END, CLAP_EVENT_PARAM_VALUE, CLAP_TRANSPORT_HAS_BEATS_TIMELINE,
    CLAP_TRANSPORT_HAS_SECONDS_TIMELINE, CLAP_TRANSPORT_HAS_TEMPO,
    CLAP_TRANSPORT_HAS_TIME_SIGNATURE, CLAP_TRANSPORT_IS_LOOP_ACTIVE, CLAP_TRANSPORT_IS_PLAYING,
    CLAP_TRANSPORT_IS_RECORDING, clap_event_header, clap_event_midi, clap_event_note,
    clap_event_param_gesture, clap_event_param_value, clap_event_transport, clap_event_type,
    clap_input_events, clap_output_events,
};
use clap_sys::ext::audio_ports::{
    CLAP_AUDIO_PORT_IS_MAIN, CLAP_EXT_AUDIO_PORTS, CLAP_PORT_MONO, CLAP_PORT_STEREO,
    clap_audio_port_info, clap_plugin_audio_ports,
};
use clap_sys::ext::audio_ports_config::{
    CLAP_EXT_AUDIO_PORTS_CONFIG, clap_audio_ports_config, clap_plugin_audio_ports_config,
};
use clap_sys::ext::gui::{
    CLAP_EXT_GUI, CLAP_WINDOW_API_COCOA, CLAP_WINDOW_API_WIN32, CLAP_WINDOW_API_X11, clap_host_gui,
    clap_plugin_gui, clap_window,
};
use clap_sys::ext::latency::{CLAP_EXT_LATENCY, clap_host_latency, clap_plugin_latency};
use clap_sys::ext::params::{
    CLAP_EXT_PARAMS, CLAP_PARAM_IS_AUTOMATABLE, CLAP_PARAM_IS_BYPASS, CLAP_PARAM_IS_HIDDEN,
    CLAP_PARAM_IS_READONLY, CLAP_PARAM_IS_STEPPED, CLAP_PARAM_RESCAN_VALUES, clap_host_params,
    clap_param_info, clap_plugin_params,
};
use clap_sys::ext::remote_controls::{
    CLAP_EXT_REMOTE_CONTROLS, CLAP_EXT_REMOTE_CONTROLS_COMPAT, CLAP_REMOTE_CONTROLS_COUNT,
    clap_plugin_remote_controls, clap_remote_controls_page,
};
use clap_sys::ext::state::{CLAP_EXT_STATE, clap_plugin_state};
use clap_sys::factory::plugin_factory::{CLAP_PLUGIN_FACTORY_ID, clap_plugin_factory};
use clap_sys::host::clap_host;
use clap_sys::id::{CLAP_INVALID_ID, clap_id};
use clap_sys::plugin::{clap_plugin, clap_plugin_descriptor};
use clap_sys::plugin_features::{
    CLAP_PLUGIN_FEATURE_ANALYZER, CLAP_PLUGIN_FEATURE_AUDIO_EFFECT, CLAP_PLUGIN_FEATURE_INSTRUMENT,
    CLAP_PLUGIN_FEATURE_NOTE_EFFECT,
};
use clap_sys::process::{
    CLAP_PROCESS_CONTINUE, CLAP_PROCESS_ERROR, CLAP_PROCESS_TAIL, clap_process, clap_process_status,
};
use clap_sys::stream::{clap_istream, clap_ostream};
use clap_sys::string_sizes::{CLAP_NAME_SIZE, CLAP_PATH_SIZE};
use clap_sys::version::CLAP_VERSION;

// ---------------------------------------------------------------------------
// Export macro
// ---------------------------------------------------------------------------

/// Hidden re-export so [`export!`] can name clap-sys types without a
/// direct plugin dependency on `clap-sys`.
#[doc(hidden)]
pub use clap_sys as __clap_sys;

/// Export `$logic` ([`PluginLogic`]) as this cdylib's CLAP entry point.
#[macro_export]
macro_rules! export {
    ($logic:ty) => {
        #[allow(non_upper_case_globals)]
        #[unsafe(no_mangle)]
        pub static clap_entry: $crate::__clap_sys::entry::clap_plugin_entry =
            $crate::__clap_sys::entry::clap_plugin_entry {
                clap_version: $crate::__clap_sys::version::CLAP_VERSION,
                init: Some($crate::entry_init),
                deinit: Some($crate::entry_deinit),
                get_factory: Some($crate::get_factory::<$logic>),
            };
    };
}

// ---------------------------------------------------------------------------
// Entry
// ---------------------------------------------------------------------------

#[must_use]
pub unsafe extern "C" fn entry_init(_plugin_path: *const c_char) -> bool {
    true
}

pub unsafe extern "C" fn entry_deinit() {}

#[must_use]
pub unsafe extern "C" fn get_factory<L: PluginLogic>(factory_id: *const c_char) -> *const c_void {
    if factory_id.is_null() {
        return ptr::null();
    }
    let id = unsafe { CStr::from_ptr(factory_id) };
    if id == CLAP_PLUGIN_FACTORY_ID {
        factory::<L>() as *const clap_plugin_factory as *const c_void
    } else {
        ptr::null()
    }
}

fn factory<L: PluginLogic>() -> &'static clap_plugin_factory {
    // Monomorphized static: unique per L.
    static CELL: OnceLock<clap_plugin_factory> = OnceLock::new();
    CELL.get_or_init(|| clap_plugin_factory {
        get_plugin_count: Some(factory_get_plugin_count),
        get_plugin_descriptor: Some(factory_get_plugin_descriptor::<L>),
        create_plugin: Some(factory_create_plugin::<L>),
    })
}

unsafe extern "C" fn factory_get_plugin_count(_factory: *const clap_plugin_factory) -> u32 {
    1
}

unsafe extern "C" fn factory_get_plugin_descriptor<L: PluginLogic>(
    _factory: *const clap_plugin_factory,
    index: u32,
) -> *const clap_plugin_descriptor {
    if index != 0 {
        return ptr::null();
    }
    descriptor::<L>()
}

unsafe extern "C" fn factory_create_plugin<L: PluginLogic>(
    _factory: *const clap_plugin_factory,
    host: *const clap_host,
    plugin_id: *const c_char,
) -> *const clap_plugin {
    if plugin_id.is_null() {
        return ptr::null();
    }
    let want = unsafe { CStr::from_ptr(plugin_id) };
    let info = L::info();
    let Ok(id) = CString::new(info.clap_id) else {
        return ptr::null();
    };
    if want.to_bytes() != id.as_bytes() {
        return ptr::null();
    }

    let params = Arc::new(L::Params::default());
    let desc = descriptor::<L>();

    let instance = Box::new(Instance::<L> {
        host,
        params,
        editor: None,
        param_events: Arc::new(ParamEventQueue::default()),
        state: None,
        sample_rate: 44_100.0,
        max_frames: 0,
        active: false,
        layout_index: 0,
        latency_cache: AtomicU32::new(0),
        latency_dirty: AtomicBool::new(false),
    });

    let plugin = Box::new(clap_plugin {
        desc,
        plugin_data: Box::into_raw(instance) as *mut c_void,
        init: Some(plugin_init::<L>),
        destroy: Some(plugin_destroy::<L>),
        activate: Some(plugin_activate::<L>),
        deactivate: Some(plugin_deactivate::<L>),
        start_processing: Some(plugin_start_processing::<L>),
        stop_processing: Some(plugin_stop_processing::<L>),
        reset: Some(plugin_reset::<L>),
        process: Some(plugin_process::<L>),
        get_extension: Some(plugin_get_extension::<L>),
        on_main_thread: Some(plugin_on_main_thread::<L>),
    });

    Box::into_raw(plugin)
}

// ---------------------------------------------------------------------------
// Descriptor (leaked, pointer-stable)
// ---------------------------------------------------------------------------

struct DescStorage {
    id: CString,
    name: CString,
    vendor: CString,
    url: CString,
    version: CString,
    description: CString,
    feature_ptrs: Vec<*const c_char>,
    desc: clap_plugin_descriptor,
}

fn descriptor<L: PluginLogic>() -> &'static clap_plugin_descriptor {
    // Unique static per monomorphization of L.
    static CELL: OnceLock<&'static clap_plugin_descriptor> = OnceLock::new();
    CELL.get_or_init(|| {
        let info = L::info();
        let id = CString::new(info.clap_id).unwrap_or_default();
        let name = CString::new(info.name).unwrap_or_default();
        let vendor = CString::new(info.vendor).unwrap_or_default();
        let url = CString::new(info.url).unwrap_or_default();
        let version = CString::new(info.version).unwrap_or_default();
        let description = CString::new(info.name).unwrap_or_default();

        let feature: &CStr = match info.category {
            PluginCategory::Instrument => CLAP_PLUGIN_FEATURE_INSTRUMENT,
            PluginCategory::Analyzer => CLAP_PLUGIN_FEATURE_ANALYZER,
            PluginCategory::NoteEffect => CLAP_PLUGIN_FEATURE_NOTE_EFFECT,
            PluginCategory::Effect => CLAP_PLUGIN_FEATURE_AUDIO_EFFECT,
        };

        let storage = Box::leak(Box::new(DescStorage {
            id,
            name,
            vendor,
            url,
            version,
            description,
            feature_ptrs: Vec::new(),
            desc: unsafe { std::mem::zeroed() },
        }));

        storage.feature_ptrs = vec![feature.as_ptr(), ptr::null()];
        storage.desc = clap_plugin_descriptor {
            clap_version: CLAP_VERSION,
            id: storage.id.as_ptr(),
            name: storage.name.as_ptr(),
            vendor: storage.vendor.as_ptr(),
            url: storage.url.as_ptr(),
            manual_url: storage.url.as_ptr(),
            support_url: storage.url.as_ptr(),
            version: storage.version.as_ptr(),
            description: storage.description.as_ptr(),
            features: storage.feature_ptrs.as_ptr(),
        };
        &storage.desc
    })
}

// ---------------------------------------------------------------------------
// Instance
// ---------------------------------------------------------------------------

struct Instance<L: PluginLogic> {
    host: *const clap_host,
    params: Arc<L::Params>,
    /// Created on the main thread in `plugin_init`; `None` = no GUI.
    editor: Option<Box<dyn Editor>>,
    /// GUI → host param events, drained in process/flush.
    param_events: Arc<ParamEventQueue>,
    state: Option<L::DspState>,
    sample_rate: f64,
    max_frames: u32,
    active: bool,
    /// Index into `L::bus_layouts()` selected via `audio-ports-config`.
    layout_index: usize,
    /// Last latency reported to the host (audio/main shared).
    latency_cache: AtomicU32,
    /// Set on the audio thread when latency changes; drained on main thread.
    latency_dirty: AtomicBool,
}

impl<L: PluginLogic> Instance<L> {
    fn selected_layout(&self) -> BusLayout {
        let layouts = L::bus_layouts();
        layout_at(&layouts, self.layout_index)
    }

    /// Snapshot `PluginLogic::latency` into the cache. Returns `true` if it changed.
    fn update_latency_cache(&self) -> bool {
        let Some(state) = self.state.as_ref() else {
            return false;
        };
        let new = L::latency(state);
        let old = self.latency_cache.swap(new, Ordering::Relaxed);
        old != new
    }

    unsafe fn from_plugin<'a>(plugin: *const clap_plugin) -> Option<&'a mut Self> {
        if plugin.is_null() {
            return None;
        }
        let data = unsafe { (*plugin).plugin_data as *mut Self };
        if data.is_null() {
            return None;
        }
        Some(unsafe { &mut *data })
    }
}

/// Call `clap_host_latency.changed` if the host exposes it (main thread).
fn host_latency_changed(host: *const clap_host) {
    if host.is_null() {
        return;
    }
    let Some(get) = (unsafe { (*host).get_extension }) else {
        return;
    };
    let ext = unsafe { get(host, CLAP_EXT_LATENCY.as_ptr()) };
    if ext.is_null() {
        return;
    }
    let host_lat = unsafe { &*(ext as *const clap_host_latency) };
    if let Some(changed) = host_lat.changed {
        unsafe { changed(host) };
    }
}

/// Ask the host to call `on_main_thread` soon (audio-thread safe).
fn host_request_callback(host: *const clap_host) {
    if host.is_null() {
        return;
    }
    if let Some(req) = unsafe { (*host).request_callback } {
        unsafe { req(host) };
    }
}

/// Host restart so PDC can re-read latency after a mid-run change.
fn host_request_restart(host: *const clap_host) {
    if host.is_null() {
        return;
    }
    if let Some(req) = unsafe { (*host).request_restart } {
        unsafe { req(host) };
    }
}

// ---------------------------------------------------------------------------
// Plugin callbacks
// ---------------------------------------------------------------------------

unsafe extern "C" fn plugin_init<L: PluginLogic>(plugin: *const clap_plugin) -> bool {
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return false;
    };
    // GUI factory runs on the main thread; gui.create only checks presence.
    inst.editor = L::editor(Arc::clone(&inst.params));
    true
}

unsafe extern "C" fn plugin_destroy<L: PluginLogic>(plugin: *const clap_plugin) {
    if plugin.is_null() {
        return;
    }
    // Drop (editor teardown, GPU) must not unwind across the C ABI.
    host_callback("CLAP", "destroy", || {
        let plugin = unsafe { Box::from_raw(plugin.cast_mut()) };
        if !plugin.plugin_data.is_null() {
            drop(unsafe { Box::from_raw(plugin.plugin_data as *mut Instance<L>) });
        }
    });
}

unsafe extern "C" fn plugin_activate<L: PluginLogic>(
    plugin: *const clap_plugin,
    sample_rate: f64,
    _min_frames: u32,
    max_frames: u32,
) -> bool {
    host_callback_with("CLAP", "activate", false, || {
        let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
            return false;
        };
        inst.sample_rate = sample_rate;
        inst.max_frames = max_frames;
        let layout = inst.selected_layout();
        let config = AudioConfig::new(sample_rate, max_frames as usize)
            .with_channels(layout.main_input_channels(), layout.main_output_channels());
        let mut state = L::init(&inst.params, sample_rate);
        L::reset(&mut state, &inst.params, &config);
        inst.params.set_sample_rate(sample_rate);
        let latency = L::latency(&state);
        inst.state = Some(state);
        inst.latency_cache.store(latency, Ordering::Relaxed);
        inst.latency_dirty.store(false, Ordering::Relaxed);
        inst.active = true;
        // Spec: latency may change during activate — tell the host now (main thread).
        host_latency_changed(inst.host);
        true
    })
}

unsafe extern "C" fn plugin_deactivate<L: PluginLogic>(plugin: *const clap_plugin) {
    host_callback("CLAP", "deactivate", || {
        if let Some(inst) = unsafe { Instance::<L>::from_plugin(plugin) } {
            inst.active = false;
            inst.state = None;
        }
    });
}

unsafe extern "C" fn plugin_start_processing<L: PluginLogic>(plugin: *const clap_plugin) -> bool {
    unsafe { Instance::<L>::from_plugin(plugin) }.is_some()
}

unsafe extern "C" fn plugin_stop_processing<L: PluginLogic>(_plugin: *const clap_plugin) {}

unsafe extern "C" fn plugin_reset<L: PluginLogic>(plugin: *const clap_plugin) {
    host_callback("CLAP", "reset", || {
        let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
            return;
        };
        let layout = inst.selected_layout();
        let config = AudioConfig::new(inst.sample_rate, inst.max_frames as usize)
            .with_channels(layout.main_input_channels(), layout.main_output_channels());
        if let Some(state) = inst.state.as_mut() {
            L::reset(state, &inst.params, &config);
            inst.params.snap_smoothers();
        }
        if inst.update_latency_cache() {
            // Reset runs while active; free-audio wants restart for mid-run changes.
            host_latency_changed(inst.host);
            host_request_restart(inst.host);
        }
    });
}

unsafe extern "C" fn plugin_process<L: PluginLogic>(
    plugin: *const clap_plugin,
    process: *const clap_process,
) -> clap_process_status {
    // Author `process` + buffer glue: never unwind into the host.
    host_callback_with("CLAP", "process", CLAP_PROCESS_ERROR, || {
        let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
            return CLAP_PROCESS_ERROR;
        };
        if process.is_null() || !inst.active {
            return CLAP_PROCESS_ERROR;
        }
        let process = unsafe { &*process };
        let frames = process.frames_count as usize;
        if frames == 0 {
            return CLAP_PROCESS_CONTINUE;
        }

        let mut midi = MidiBuffer::with_capacity(32);
        if !process.in_events.is_null() {
            unsafe { apply_input_events(&*inst.params, process.in_events, &mut midi) };
        }
        unsafe { emit_param_events(&inst.param_events, process.out_events) };

        let transport = if process.transport.is_null() {
            None
        } else {
            Some(map_transport(unsafe { &*process.transport }))
        };

        let Some(state) = inst.state.as_mut() else {
            return CLAP_PROCESS_ERROR;
        };

        let out_port = if process.audio_outputs_count > 0 && !process.audio_outputs.is_null() {
            unsafe { &*process.audio_outputs }
        } else {
            return CLAP_PROCESS_ERROR;
        };
        let in_port = if process.audio_inputs_count > 0 && !process.audio_inputs.is_null() {
            Some(unsafe { &*process.audio_inputs })
        } else {
            None
        };

        let ch_out = out_port.channel_count as usize;
        if ch_out == 0 || out_port.data32.is_null() {
            return CLAP_PROCESS_ERROR;
        }

        // Own channel buffers for the block (safe AudioBuffer construction).
        let mut owned_in: Vec<Vec<f32>> = Vec::with_capacity(ch_out);
        let mut owned_out: Vec<Vec<f32>> = Vec::with_capacity(ch_out);
        let mut host_out: Vec<*mut f32> = Vec::with_capacity(ch_out);

        for c in 0..ch_out {
            let op = unsafe { *out_port.data32.add(c) };
            if op.is_null() {
                return CLAP_PROCESS_ERROR;
            }
            host_out.push(op);

            let in_data = in_port
                .filter(|p| !p.data32.is_null() && (c as u32) < p.channel_count)
                .and_then(|p| {
                    let ip = unsafe { *p.data32.add(c) };
                    if ip.is_null() {
                        None
                    } else {
                        Some(unsafe { std::slice::from_raw_parts(ip, frames) })
                    }
                });

            if let Some(s) = in_data {
                owned_in.push(s.to_vec());
                owned_out.push(s.to_vec());
            } else {
                owned_in.push(vec![0.0; frames]);
                owned_out.push(vec![0.0; frames]);
            }
        }

        let in_refs: Vec<&[f32]> = owned_in.iter().map(Vec::as_slice).collect();
        let mut out_refs: Vec<&mut [f32]> = owned_out.iter_mut().map(Vec::as_mut_slice).collect();

        let status = {
            let mut buffer = AudioBuffer::from_slices_checked(&in_refs, &mut out_refs, frames);
            let mut ctx = ProcessContext::new(inst.sample_rate, frames)
                .with_process_mode(ProcessMode::Realtime)
                .with_midi(midi);
            ctx.transport = transport;
            L::process(state, &inst.params, &mut buffer, &mut ctx)
        };

        for (c, host_ptr) in host_out.iter().enumerate() {
            let dst = unsafe { std::slice::from_raw_parts_mut(*host_ptr, frames) };
            dst.copy_from_slice(&owned_out[c]);
        }

        // PDC: if latency moved, schedule main-thread notify (never call host
        // latency.changed from the audio thread).
        if inst.update_latency_cache() {
            inst.latency_dirty.store(true, Ordering::Relaxed);
            host_request_callback(inst.host);
        }

        match status {
            ProcessStatus::Continue => CLAP_PROCESS_CONTINUE,
            ProcessStatus::TailFinished => CLAP_PROCESS_TAIL,
            ProcessStatus::Error => CLAP_PROCESS_ERROR,
        }
    })
}

unsafe fn apply_input_events(
    params: &dyn Params,
    in_events: *const clap_input_events,
    midi: &mut MidiBuffer,
) {
    midi.clear();
    let ev = unsafe { &*in_events };
    let Some(size_fn) = ev.size else {
        return;
    };
    let Some(get_fn) = ev.get else {
        return;
    };
    let n = unsafe { size_fn(ev) };
    for i in 0..n {
        let hdr = unsafe { get_fn(ev, i) };
        if hdr.is_null() {
            continue;
        }
        let header = unsafe { &*hdr };
        if header.space_id != CLAP_CORE_EVENT_SPACE_ID {
            continue;
        }
        match header.type_ {
            CLAP_EVENT_PARAM_VALUE => {
                let pev = unsafe { &*(hdr as *const clap_event_param_value) };
                params.set_plain(pev.param_id, pev.value);
            }
            CLAP_EVENT_NOTE_ON | CLAP_EVENT_NOTE_OFF | CLAP_EVENT_NOTE_CHOKE => {
                if let Some(msg) = clap_note_to_midi(header.type_, hdr) {
                    midi.push(header.time, msg);
                }
            }
            CLAP_EVENT_MIDI => {
                let m = unsafe { &*(hdr as *const clap_event_midi) };
                midi.push(
                    header.time,
                    MidiMessage::raw(m.data[0], m.data[1], m.data[2]),
                );
            }
            _ => {}
        }
    }
}

/// Map CLAP note events into channel MIDI (velocity 0..=127).
fn clap_note_to_midi(type_: clap_event_type, hdr: *const clap_event_header) -> Option<MidiMessage> {
    let n = unsafe { &*(hdr as *const clap_event_note) };
    if n.key < 0 || n.key > 127 {
        return None;
    }
    let channel = if n.channel < 0 {
        0
    } else {
        (n.channel as u8) & 0x0F
    };
    let key = n.key as u8;
    let velocity = (n.velocity * 127.0).round().clamp(0.0, 127.0) as u8;
    match type_ {
        CLAP_EVENT_NOTE_ON => Some(MidiMessage::note_on(channel, key, velocity.max(1))),
        CLAP_EVENT_NOTE_OFF | CLAP_EVENT_NOTE_CHOKE => {
            Some(MidiMessage::note_off(channel, key, velocity))
        }
        _ => None,
    }
}

fn event_header(type_: clap_event_type, size: u32) -> clap_event_header {
    clap_event_header {
        size,
        time: 0,
        space_id: CLAP_CORE_EVENT_SPACE_ID,
        type_,
        flags: CLAP_EVENT_IS_LIVE,
    }
}

/// Push queued GUI param events to the host's `out_events` (process/flush).
unsafe fn emit_param_events(queue: &ParamEventQueue, out: *const clap_output_events) {
    if out.is_null() {
        return;
    }
    let Some(try_push) = (unsafe { &*out }).try_push else {
        return;
    };
    for ev in queue.drain() {
        match ev {
            ParamEvent::GestureBegin(param_id) | ParamEvent::GestureEnd(param_id) => {
                let type_ = if matches!(ev, ParamEvent::GestureBegin(_)) {
                    CLAP_EVENT_PARAM_GESTURE_BEGIN
                } else {
                    CLAP_EVENT_PARAM_GESTURE_END
                };
                let e = clap_event_param_gesture {
                    header: event_header(type_, size_of::<clap_event_param_gesture>() as u32),
                    param_id,
                };
                unsafe { try_push(out, &e as *const _ as *const clap_event_header) };
            }
            ParamEvent::Value { id, plain } => {
                let e = clap_event_param_value {
                    header: event_header(
                        CLAP_EVENT_PARAM_VALUE,
                        size_of::<clap_event_param_value>() as u32,
                    ),
                    param_id: id,
                    cookie: ptr::null_mut(),
                    note_id: -1,
                    port_index: -1,
                    channel: -1,
                    key: -1,
                    value: plain,
                };
                unsafe { try_push(out, &e as *const _ as *const clap_event_header) };
            }
        }
    }
}

fn map_transport(t: &clap_event_transport) -> Transport {
    use clap_sys::fixedpoint::{CLAP_BEATTIME_FACTOR, CLAP_SECTIME_FACTOR};
    // CLAP beat/sec times are i64 fixed point (factor 2^31).
    #[allow(clippy::cast_precision_loss)]
    let beat = |v: i64| v as f64 / CLAP_BEATTIME_FACTOR as f64;
    #[allow(clippy::cast_precision_loss)]
    let sec = |v: i64| v as f64 / CLAP_SECTIME_FACTOR as f64;
    let f = t.flags;
    let beats = f & CLAP_TRANSPORT_HAS_BEATS_TIMELINE != 0;
    Transport {
        playing: f & CLAP_TRANSPORT_IS_PLAYING != 0,
        recording: f & CLAP_TRANSPORT_IS_RECORDING != 0,
        loop_active: f & CLAP_TRANSPORT_IS_LOOP_ACTIVE != 0,
        tempo: (f & CLAP_TRANSPORT_HAS_TEMPO != 0).then_some(t.tempo),
        position_beats: beats.then_some(beat(t.song_pos_beats)),
        position_seconds: (f & CLAP_TRANSPORT_HAS_SECONDS_TIMELINE != 0)
            .then_some(sec(t.song_pos_seconds)),
        loop_beats: beats.then_some((beat(t.loop_start_beats), beat(t.loop_end_beats))),
        time_signature: (f & CLAP_TRANSPORT_HAS_TIME_SIGNATURE != 0)
            .then_some((t.tsig_num, t.tsig_denom)),
        bar_number: beats.then_some(t.bar_number),
    }
}

unsafe extern "C" fn plugin_get_extension<L: PluginLogic>(
    plugin: *const clap_plugin,
    id: *const c_char,
) -> *const c_void {
    if id.is_null() {
        return ptr::null();
    }
    let id = unsafe { CStr::from_ptr(id) };
    if id == CLAP_EXT_AUDIO_PORTS {
        return audio_ports_ext::<L>() as *const _ as *const c_void;
    }
    if id == CLAP_EXT_AUDIO_PORTS_CONFIG && L::bus_layouts().len() > 1 {
        return audio_ports_config_ext::<L>() as *const _ as *const c_void;
    }
    if id == CLAP_EXT_PARAMS {
        return params_ext::<L>() as *const _ as *const c_void;
    }
    if id == CLAP_EXT_REMOTE_CONTROLS || id == CLAP_EXT_REMOTE_CONTROLS_COMPAT {
        return remote_controls_ext::<L>() as *const _ as *const c_void;
    }
    if id == CLAP_EXT_LATENCY {
        return latency_ext::<L>() as *const _ as *const c_void;
    }
    if id == CLAP_EXT_STATE {
        return state_ext::<L>() as *const _ as *const c_void;
    }
    if id == CLAP_EXT_GUI {
        // No GUI extension when the plugin has no editor.
        let has_editor =
            unsafe { Instance::<L>::from_plugin(plugin) }.is_some_and(|inst| inst.editor.is_some());
        if has_editor {
            return gui_ext::<L>() as *const _ as *const c_void;
        }
        return ptr::null();
    }
    ptr::null()
}

unsafe extern "C" fn plugin_on_main_thread<L: PluginLogic>(plugin: *const clap_plugin) {
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return;
    };
    if !inst.latency_dirty.swap(false, Ordering::Relaxed) {
        return;
    }
    // Mid-run latency change: notify + restart so the host re-activates
    // and re-reads get() for PDC (free-audio: change only during activate,
    // otherwise request_restart).
    host_latency_changed(inst.host);
    host_request_restart(inst.host);
}

// ---------------------------------------------------------------------------
// latency (PDC)
// ---------------------------------------------------------------------------

fn latency_ext<L: PluginLogic>() -> &'static clap_plugin_latency {
    static CELL: OnceLock<clap_plugin_latency> = OnceLock::new();
    CELL.get_or_init(|| clap_plugin_latency {
        get: Some(latency_get::<L>),
    })
}

unsafe extern "C" fn latency_get<L: PluginLogic>(plugin: *const clap_plugin) -> u32 {
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return 0;
    };
    inst.latency_cache.load(Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// audio-ports (main in/out from selected BusLayout)
// ---------------------------------------------------------------------------

fn clap_port_type(channels: ChannelConfig) -> *const c_char {
    match channels {
        ChannelConfig::Mono => CLAP_PORT_MONO.as_ptr(),
        ChannelConfig::Stereo => CLAP_PORT_STEREO.as_ptr(),
    }
}

fn audio_ports_ext<L: PluginLogic>() -> &'static clap_plugin_audio_ports {
    static CELL: OnceLock<clap_plugin_audio_ports> = OnceLock::new();
    CELL.get_or_init(|| clap_plugin_audio_ports {
        count: Some(audio_ports_count::<L>),
        get: Some(audio_ports_get::<L>),
    })
}

unsafe extern "C" fn audio_ports_count<L: PluginLogic>(
    plugin: *const clap_plugin,
    is_input: bool,
) -> u32 {
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return 0;
    };
    let layout = inst.selected_layout();
    if is_input {
        u32::from(layout.main_in.is_some())
    } else {
        1
    }
}

unsafe extern "C" fn audio_ports_get<L: PluginLogic>(
    plugin: *const clap_plugin,
    index: u32,
    is_input: bool,
    info: *mut clap_audio_port_info,
) -> bool {
    if index != 0 || info.is_null() {
        return false;
    }
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return false;
    };
    let layout = inst.selected_layout();
    let (channels, name) = if is_input {
        let Some(ch) = layout.main_in else {
            return false;
        };
        (ch, "Input")
    } else {
        (layout.main_out, "Output")
    };
    let info = unsafe { &mut *info };
    info.id = u32::from(!is_input);
    write_name(&mut info.name, name);
    info.flags = CLAP_AUDIO_PORT_IS_MAIN;
    info.channel_count = channels.channel_count();
    info.port_type = clap_port_type(channels);
    info.in_place_pair = CLAP_INVALID_ID;
    true
}

// ---------------------------------------------------------------------------
// audio-ports-config (when bus_layouts().len() > 1)
// ---------------------------------------------------------------------------

fn audio_ports_config_ext<L: PluginLogic>() -> &'static clap_plugin_audio_ports_config {
    static CELL: OnceLock<clap_plugin_audio_ports_config> = OnceLock::new();
    CELL.get_or_init(|| clap_plugin_audio_ports_config {
        count: Some(audio_ports_config_count::<L>),
        get: Some(audio_ports_config_get::<L>),
        select: Some(audio_ports_config_select::<L>),
    })
}

unsafe extern "C" fn audio_ports_config_count<L: PluginLogic>(_plugin: *const clap_plugin) -> u32 {
    L::bus_layouts().len() as u32
}

unsafe extern "C" fn audio_ports_config_get<L: PluginLogic>(
    _plugin: *const clap_plugin,
    index: u32,
    config: *mut clap_audio_ports_config,
) -> bool {
    if config.is_null() {
        return false;
    }
    let layouts = L::bus_layouts();
    let Some(layout) = layouts.get(index as usize).copied() else {
        return false;
    };
    let out = unsafe { &mut *config };
    out.id = index;
    write_name(&mut out.name, &layout.config_name());
    out.input_port_count = u32::from(layout.main_in.is_some());
    out.output_port_count = 1;
    out.has_main_input = layout.main_in.is_some();
    out.main_input_channel_count = layout.main_input_channels();
    out.main_input_port_type = layout.main_in.map_or(ptr::null(), clap_port_type);
    out.has_main_output = true;
    out.main_output_channel_count = layout.main_output_channels();
    out.main_output_port_type = clap_port_type(layout.main_out);
    true
}

unsafe extern "C" fn audio_ports_config_select<L: PluginLogic>(
    plugin: *const clap_plugin,
    config_id: clap_id,
) -> bool {
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return false;
    };
    if inst.active {
        // Spec: select only while deactivated.
        return false;
    }
    let n = L::bus_layouts().len();
    if (config_id as usize) >= n {
        return false;
    }
    inst.layout_index = config_id as usize;
    true
}

// ---------------------------------------------------------------------------
// remote-controls (device pages from ParamInfo.group)
// ---------------------------------------------------------------------------
//
// free-audio: clap.remote-controls/2 — up to 8 param slots per page, sections
// cycle through pages. We map `ParamInfo.group` to pages:
// - empty group → no remote page (host generic param list still has the param)
// - `"Section/Page"` → section + page names (first `/` only)
// - `"Section"` alone → section and page share the same name
// - >8 params in one group → multiple pages (chunk index in stable page_id)
// - HIDDEN / READONLY never consume a scarce hardware slot

/// Split `group` on the first `/` into `(section_name, page_name)`.
fn split_group(group: &str) -> (&str, &str) {
    match group.split_once('/') {
        Some((section, page)) => (section, page),
        None => (group, group),
    }
}

/// Chunk params into remote-control pages (≤8 ids each), one run per
/// distinct non-empty `ParamInfo.group` (declaration order).
fn remote_control_pages(infos: &[ParamInfo]) -> Vec<(&str, Vec<u32>)> {
    let mut pages: Vec<(&str, Vec<u32>)> = Vec::new();
    for info in infos {
        if info.group.is_empty()
            || info
                .flags
                .intersects(ParamFlags::HIDDEN | ParamFlags::READONLY)
        {
            continue;
        }
        let existing = pages
            .iter()
            .rposition(|page| page.0 == info.group && page.1.len() < CLAP_REMOTE_CONTROLS_COUNT);
        match existing {
            Some(idx) => pages[idx].1.push(info.id),
            None => pages.push((info.group, vec![info.id])),
        }
    }
    pages
}

/// Stable FNV-1a page id from group name + chunk index (host may persist
/// the user's last page; must not depend on process-local hashers).
fn remote_controls_page_id(group: &str, chunk_index: usize) -> clap_id {
    let mut hash: u32 = 0x811c_9dc5;
    for &b in group.as_bytes() {
        hash ^= u32::from(b);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    for &b in &chunk_index.to_le_bytes() {
        hash ^= u32::from(b);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

fn remote_controls_ext<L: PluginLogic>() -> &'static clap_plugin_remote_controls {
    static CELL: OnceLock<clap_plugin_remote_controls> = OnceLock::new();
    CELL.get_or_init(|| clap_plugin_remote_controls {
        count: Some(remote_controls_count::<L>),
        get: Some(remote_controls_get::<L>),
    })
}

unsafe extern "C" fn remote_controls_count<L: PluginLogic>(plugin: *const clap_plugin) -> u32 {
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return 0;
    };
    let infos = inst.params.param_infos();
    remote_control_pages(&infos).len() as u32
}

unsafe extern "C" fn remote_controls_get<L: PluginLogic>(
    plugin: *const clap_plugin,
    page_index: u32,
    out: *mut clap_remote_controls_page,
) -> bool {
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return false;
    };
    if out.is_null() {
        return false;
    }
    let infos = inst.params.param_infos();
    let pages = remote_control_pages(&infos);
    let page_index = page_index as usize;
    if page_index >= pages.len() {
        return false;
    }
    let group = pages[page_index].0;
    let ids = &pages[page_index].1;
    let chunk_index = pages[..page_index]
        .iter()
        .filter(|page| page.0 == group)
        .count();
    let (section_name, page_name) = split_group(group);

    let out = unsafe { &mut *out };
    write_name(&mut out.section_name, section_name);
    out.page_id = remote_controls_page_id(group, chunk_index);
    write_name(&mut out.page_name, page_name);
    out.param_ids = [CLAP_INVALID_ID; CLAP_REMOTE_CONTROLS_COUNT];
    for (slot, &id) in out.param_ids.iter_mut().zip(ids.iter()) {
        *slot = id;
    }
    out.is_for_preset = false;
    true
}

// ---------------------------------------------------------------------------
// params
// ---------------------------------------------------------------------------

fn params_ext<L: PluginLogic>() -> &'static clap_plugin_params {
    static CELL: OnceLock<clap_plugin_params> = OnceLock::new();
    CELL.get_or_init(|| clap_plugin_params {
        count: Some(params_count::<L>),
        get_info: Some(params_get_info::<L>),
        get_value: Some(params_get_value::<L>),
        value_to_text: Some(params_value_to_text::<L>),
        text_to_value: Some(params_text_to_value::<L>),
        flush: Some(params_flush::<L>),
    })
}

unsafe extern "C" fn params_count<L: PluginLogic>(plugin: *const clap_plugin) -> u32 {
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return 0;
    };
    inst.params.count() as u32
}

unsafe extern "C" fn params_get_info<L: PluginLogic>(
    plugin: *const clap_plugin,
    param_index: u32,
    param_info: *mut clap_param_info,
) -> bool {
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return false;
    };
    if param_info.is_null() {
        return false;
    }
    let infos = inst.params.param_infos();
    let Some(meta) = infos.get(param_index as usize) else {
        return false;
    };
    let out = unsafe { &mut *param_info };
    out.id = meta.id;
    out.flags = map_param_flags(meta.flags, &meta.range);
    out.cookie = ptr::null_mut();
    write_name(&mut out.name, meta.name);
    write_path(&mut out.module, meta.group);
    out.min_value = meta.range.min();
    out.max_value = meta.range.max();
    out.default_value = meta.default_plain;
    true
}

unsafe extern "C" fn params_get_value<L: PluginLogic>(
    plugin: *const clap_plugin,
    param_id: u32,
    out_value: *mut f64,
) -> bool {
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return false;
    };
    if out_value.is_null() {
        return false;
    }
    match inst.params.get_plain(param_id) {
        Some(v) => {
            unsafe { *out_value = v };
            true
        }
        None => false,
    }
}

unsafe extern "C" fn params_value_to_text<L: PluginLogic>(
    plugin: *const clap_plugin,
    param_id: u32,
    value: f64,
    out_buffer: *mut c_char,
    out_buffer_capacity: u32,
) -> bool {
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return false;
    };
    if out_buffer.is_null() || out_buffer_capacity == 0 {
        return false;
    }
    let text = inst
        .params
        .format_value(param_id, value)
        .unwrap_or_else(|| format!("{value:.2}"));
    write_c_buf(out_buffer, out_buffer_capacity as usize, &text)
}

unsafe extern "C" fn params_text_to_value<L: PluginLogic>(
    plugin: *const clap_plugin,
    param_id: u32,
    param_value_text: *const c_char,
    out_value: *mut f64,
) -> bool {
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return false;
    };
    if param_value_text.is_null() || out_value.is_null() {
        return false;
    }
    let text = unsafe { CStr::from_ptr(param_value_text) }.to_string_lossy();
    match inst.params.parse_value(param_id, &text) {
        Some(v) => {
            unsafe { *out_value = v };
            true
        }
        None => false,
    }
}

unsafe extern "C" fn params_flush<L: PluginLogic>(
    plugin: *const clap_plugin,
    in_: *const clap_input_events,
    out: *const clap_output_events,
) {
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return;
    };
    if !in_.is_null() {
        // Flush is param-only; MIDI discarded (no process block).
        let mut midi = MidiBuffer::new();
        unsafe { apply_input_events(&*inst.params, in_, &mut midi) };
    }
    unsafe { emit_param_events(&inst.param_events, out) };
}

// ---------------------------------------------------------------------------
// gui
// ---------------------------------------------------------------------------

/// Host bridge for the editor: param store + `clap_host_gui.request_resize`.
///
/// Param edits land in the store immediately (audio reads them next block)
/// and are queued as CLAP gesture/value events for the host's automation.
struct ClapBridge {
    params: Arc<dyn Params>,
    events: Arc<ParamEventQueue>,
    host: *const clap_host,
}

// SAFETY: used from the GUI thread only; `host` is valid for the plugin
// lifetime and `clap_host_gui` calls are main-thread per CLAP spec.
unsafe impl Send for ClapBridge {}
unsafe impl Sync for ClapBridge {}

impl ClapBridge {
    fn host_gui(&self) -> Option<&clap_host_gui> {
        if self.host.is_null() {
            return None;
        }
        let get = unsafe { (*self.host).get_extension? };
        let ext = unsafe { get(self.host, CLAP_EXT_GUI.as_ptr()) };
        if ext.is_null() {
            None
        } else {
            Some(unsafe { &*(ext as *const clap_host_gui) })
        }
    }
}

impl EditorBridge for ClapBridge {
    fn begin_edit(&self, id: u32) {
        self.events.push(ParamEvent::GestureBegin(id));
    }

    fn set_param(&self, id: u32, normalized: f64) {
        self.params.set_normalized(id, normalized);
        if let Some(plain) = self.params.get_plain(id) {
            self.events.push(ParamEvent::Value { id, plain });
        }
    }

    fn end_edit(&self, id: u32) {
        self.events.push(ParamEvent::GestureEnd(id));
    }

    fn get_param(&self, id: u32) -> f64 {
        self.params.get_normalized(id).unwrap_or(0.0)
    }

    fn get_param_plain(&self, id: u32) -> f64 {
        self.params.get_plain(id).unwrap_or(0.0)
    }

    fn request_resize(&self, w: u32, h: u32) -> bool {
        let Some(rr) = self.host_gui().and_then(|gui| gui.request_resize) else {
            return false;
        };
        unsafe { rr(self.host, w, h) }
    }
}

fn platform_window_api() -> &'static CStr {
    #[cfg(target_os = "windows")]
    {
        CLAP_WINDOW_API_WIN32
    }
    #[cfg(target_os = "macos")]
    {
        CLAP_WINDOW_API_COCOA
    }
    #[cfg(target_os = "linux")]
    {
        CLAP_WINDOW_API_X11
    }
}

fn gui_ext<L: PluginLogic>() -> &'static clap_plugin_gui {
    static CELL: OnceLock<clap_plugin_gui> = OnceLock::new();
    CELL.get_or_init(|| clap_plugin_gui {
        is_api_supported: Some(gui_is_api_supported::<L>),
        get_preferred_api: Some(gui_get_preferred_api::<L>),
        create: Some(gui_create::<L>),
        destroy: Some(gui_destroy::<L>),
        set_scale: Some(gui_set_scale::<L>),
        get_size: Some(gui_get_size::<L>),
        can_resize: Some(gui_can_resize::<L>),
        get_resize_hints: Some(gui_get_resize_hints),
        adjust_size: Some(gui_adjust_size::<L>),
        set_size: Some(gui_set_size::<L>),
        set_parent: Some(gui_set_parent::<L>),
        set_transient: Some(gui_set_transient),
        suggest_title: Some(gui_suggest_title::<L>),
        show: Some(gui_show::<L>),
        hide: Some(gui_hide::<L>),
    })
}

unsafe extern "C" fn gui_is_api_supported<L: PluginLogic>(
    plugin: *const clap_plugin,
    api: *const c_char,
    is_floating: bool,
) -> bool {
    if is_floating || api.is_null() {
        return false;
    }
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return false;
    };
    inst.editor.is_some() && unsafe { CStr::from_ptr(api) } == platform_window_api()
}

unsafe extern "C" fn gui_get_preferred_api<L: PluginLogic>(
    plugin: *const clap_plugin,
    api: *mut *const c_char,
    is_floating: *mut bool,
) -> bool {
    let supported =
        unsafe { gui_is_api_supported::<L>(plugin, platform_window_api().as_ptr(), false) };
    if !supported || api.is_null() || is_floating.is_null() {
        return false;
    }
    unsafe {
        *api = platform_window_api().as_ptr();
        *is_floating = false;
    }
    true
}

unsafe extern "C" fn gui_create<L: PluginLogic>(
    plugin: *const clap_plugin,
    api: *const c_char,
    is_floating: bool,
) -> bool {
    unsafe { gui_is_api_supported::<L>(plugin, api, is_floating) }
}

unsafe extern "C" fn gui_destroy<L: PluginLogic>(plugin: *const clap_plugin) {
    if let Some(inst) = unsafe { Instance::<L>::from_plugin(plugin) }
        && let Some(editor) = inst.editor.as_mut()
    {
        editor.close();
    }
}

unsafe extern "C" fn gui_set_scale<L: PluginLogic>(plugin: *const clap_plugin, scale: f64) -> bool {
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return false;
    };
    let Some(editor) = inst.editor.as_mut() else {
        return false;
    };
    editor.set_scale(scale);
    true
}

unsafe extern "C" fn gui_get_size<L: PluginLogic>(
    plugin: *const clap_plugin,
    width: *mut u32,
    height: *mut u32,
) -> bool {
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return false;
    };
    let Some(editor) = inst.editor.as_mut() else {
        return false;
    };
    if width.is_null() || height.is_null() {
        return false;
    }
    let (w, h) = editor.size();
    unsafe {
        *width = w;
        *height = h;
    }
    true
}

unsafe extern "C" fn gui_can_resize<L: PluginLogic>(plugin: *const clap_plugin) -> bool {
    unsafe { Instance::<L>::from_plugin(plugin) }
        .and_then(|inst| inst.editor.as_ref())
        .is_some_and(|editor| editor.can_resize())
}

unsafe extern "C" fn gui_get_resize_hints(
    _plugin: *const clap_plugin,
    _hints: *mut clap_sys::ext::gui::clap_gui_resize_hints,
) -> bool {
    false
}

unsafe extern "C" fn gui_adjust_size<L: PluginLogic>(
    plugin: *const clap_plugin,
    width: *mut u32,
    height: *mut u32,
) -> bool {
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return false;
    };
    let Some(editor) = inst.editor.as_mut() else {
        return false;
    };
    if width.is_null() || height.is_null() {
        return false;
    }
    let (min_w, min_h) = editor.min_size();
    let (max_w, max_h) = editor.max_size();
    unsafe {
        *width = (*width).clamp(min_w.max(1), max_w);
        *height = (*height).clamp(min_h.max(1), max_h);
    }
    true
}

unsafe extern "C" fn gui_set_size<L: PluginLogic>(
    plugin: *const clap_plugin,
    width: u32,
    height: u32,
) -> bool {
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return false;
    };
    let Some(editor) = inst.editor.as_mut() else {
        return false;
    };
    editor.set_size(width, height)
}

unsafe extern "C" fn gui_set_parent<L: PluginLogic>(
    plugin: *const clap_plugin,
    window: *const clap_window,
) -> bool {
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return false;
    };
    let Some(editor) = inst.editor.as_mut() else {
        return false;
    };
    if window.is_null() || unsafe { &*window }.api.is_null() {
        return false;
    }
    let window = unsafe { &*window };
    let api = unsafe { CStr::from_ptr(window.api) };
    let parent = if api == CLAP_WINDOW_API_WIN32 {
        RawWindowHandle::Win32(unsafe { window.specific.win32 })
    } else if api == CLAP_WINDOW_API_COCOA {
        RawWindowHandle::AppKit(unsafe { window.specific.cocoa })
    } else if api == CLAP_WINDOW_API_X11 {
        RawWindowHandle::X11(u64::from(unsafe { window.specific.x11 }))
    } else {
        return false;
    };

    let params: Arc<dyn Params> = inst.params.clone();
    let ctx = PluginContext::new(params.clone())
        .with_bridge(Arc::new(ClapBridge {
            params,
            events: Arc::clone(&inst.param_events),
            host: inst.host,
        }))
        .with_sample_rate(inst.sample_rate);
    editor.open(parent, ctx);
    true
}

unsafe extern "C" fn gui_set_transient(
    _plugin: *const clap_plugin,
    _window: *const clap_window,
) -> bool {
    false
}

unsafe extern "C" fn gui_suggest_title<L: PluginLogic>(
    _plugin: *const clap_plugin,
    _title: *const c_char,
) {
}

unsafe extern "C" fn gui_show<L: PluginLogic>(plugin: *const clap_plugin) -> bool {
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return false;
    };
    let Some(editor) = inst.editor.as_mut() else {
        return false;
    };
    editor.show();
    true
}

unsafe extern "C" fn gui_hide<L: PluginLogic>(plugin: *const clap_plugin) -> bool {
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return false;
    };
    let Some(editor) = inst.editor.as_mut() else {
        return false;
    };
    editor.hide();
    true
}

// ---------------------------------------------------------------------------
// state
// ---------------------------------------------------------------------------

/// v1 blob codec lives in `aura_core::state` (shared with the VST3/LV2
/// wrappers); here we only pump bytes through the CLAP streams.
fn state_ext<L: PluginLogic>() -> &'static clap_plugin_state {
    static CELL: OnceLock<clap_plugin_state> = OnceLock::new();
    CELL.get_or_init(|| clap_plugin_state {
        save: Some(state_save::<L>),
        load: Some(state_load::<L>),
    })
}

unsafe extern "C" fn state_save<L: PluginLogic>(
    plugin: *const clap_plugin,
    stream: *const clap_ostream,
) -> bool {
    host_callback_with("CLAP", "state_save", false, || {
        let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
            return false;
        };
        if stream.is_null() {
            return false;
        }
        let Some(write) = (unsafe { &*stream }).write else {
            return false;
        };

        let blob = aura_core::encode_state(&*inst.params);

        let mut written = 0usize;
        while written < blob.len() {
            let n = unsafe {
                write(
                    stream,
                    blob.as_ptr().add(written) as *const c_void,
                    (blob.len() - written) as u64,
                )
            };
            if n <= 0 {
                return false;
            }
            written += n as usize;
        }
        true
    })
}

unsafe extern "C" fn state_load<L: PluginLogic>(
    plugin: *const clap_plugin,
    stream: *const clap_istream,
) -> bool {
    host_callback_with("CLAP", "state_load", false, || {
        let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
            return false;
        };
        if stream.is_null() {
            return false;
        }
        let Some(read) = (unsafe { &*stream }).read else {
            return false;
        };

        let mut blob = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let n = unsafe {
                read(
                    stream,
                    chunk.as_mut_ptr() as *mut c_void,
                    chunk.len() as u64,
                )
            };
            if n < 0 {
                return false;
            }
            if n == 0 {
                break;
            }
            blob.extend_from_slice(&chunk[..n as usize]);
        }

        let ok = aura_core::decode_state(&*inst.params, &blob);
        if ok {
            // Values changed behind the host's back: per CLAP spec the plugin
            // must ask for a value rescan after a state load, or the host (and
            // clap-validator's state-reproducibility tests) sees stale values.
            unsafe { request_param_rescan(inst.host) };
        }
        ok
    })
}

/// Ask the host to rescan param values (`clap_host_params.rescan`) after a
/// state load. No-op when the host is null or lacks the params extension.
unsafe fn request_param_rescan(host: *const clap_host) {
    if host.is_null() {
        return;
    }
    let Some(get) = (unsafe { &*host }).get_extension else {
        return;
    };
    let ext = unsafe { get(host, CLAP_EXT_PARAMS.as_ptr()) };
    if ext.is_null() {
        return;
    }
    let host_params = unsafe { &*(ext as *const clap_host_params) };
    if let Some(rescan) = host_params.rescan {
        unsafe { rescan(host, CLAP_PARAM_RESCAN_VALUES) };
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn map_param_flags(flags: ParamFlags, range: &ParamRange) -> u32 {
    let mut f = 0u32;
    if flags.contains(ParamFlags::AUTOMATABLE) {
        f |= CLAP_PARAM_IS_AUTOMATABLE;
    }
    if flags.contains(ParamFlags::HIDDEN) {
        f |= CLAP_PARAM_IS_HIDDEN;
    }
    if flags.contains(ParamFlags::READONLY) {
        f |= CLAP_PARAM_IS_READONLY;
    }
    if flags.contains(ParamFlags::IS_BYPASS) {
        f |= CLAP_PARAM_IS_BYPASS;
    }
    if matches!(range, ParamRange::Discrete { .. } | ParamRange::Enum { .. }) {
        f |= CLAP_PARAM_IS_STEPPED;
    }
    f
}

fn write_name(buf: &mut [c_char; CLAP_NAME_SIZE], s: &str) {
    write_fixed(buf, s);
}

fn write_path(buf: &mut [c_char; CLAP_PATH_SIZE], s: &str) {
    write_fixed(buf, s);
}

fn write_fixed<const N: usize>(buf: &mut [c_char; N], s: &str) {
    buf.fill(0);
    let bytes = s.as_bytes();
    let n = bytes.len().min(N - 1);
    for (i, b) in bytes.iter().take(n).enumerate() {
        buf[i] = *b as c_char;
    }
}

fn write_c_buf(out: *mut c_char, cap: usize, s: &str) -> bool {
    if cap == 0 {
        return false;
    }
    let bytes = s.as_bytes();
    let n = bytes.len().min(cap - 1);
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), out as *mut u8, n);
        *out.add(n) = 0;
    }
    true
}

#[cfg(test)]
mod remote_controls_tests {
    use super::{remote_control_pages, remote_controls_page_id, split_group};
    use aura_params::{ParamFlags, ParamInfo, ParamRange, ParamUnit, ParamValueKind};

    fn info(id: u32, group: &'static str) -> ParamInfo {
        ParamInfo {
            id,
            name: "p",
            short_name: "p",
            group,
            range: ParamRange::Linear { min: 0.0, max: 1.0 },
            default_plain: 0.0,
            flags: ParamFlags::empty(),
            unit: ParamUnit::None,
            kind: ParamValueKind::Float,
            midi_map: None,
            midi_channel: None,
        }
    }

    #[test]
    fn ungrouped_params_produce_no_page() {
        let infos = [info(0, ""), info(1, "")];
        assert!(remote_control_pages(&infos).is_empty());
    }

    #[test]
    fn hidden_and_readonly_grouped_params_take_no_slot() {
        let mut hidden = info(0, "EQ");
        hidden.flags = ParamFlags::HIDDEN;
        let visible = info(1, "EQ");
        let mut readonly = info(2, "EQ");
        readonly.flags = ParamFlags::READONLY;
        let infos = [hidden, visible, readonly];
        let pages = remote_control_pages(&infos);
        assert_eq!(pages, vec![("EQ", vec![1])]);
    }

    #[test]
    fn group_over_eight_params_splits_into_two_pages() {
        let infos: Vec<ParamInfo> = (0..10).map(|id| info(id, "EQ")).collect();
        let pages = remote_control_pages(&infos);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].1.len(), 8);
        assert_eq!(pages[1].1, vec![8, 9]);
    }

    #[test]
    fn distinct_groups_get_distinct_pages() {
        let mut infos: Vec<ParamInfo> = (0..3).map(|id| info(id, "EQ")).collect();
        infos.extend((3..5).map(|id| info(id, "DYN")));
        let pages = remote_control_pages(&infos);
        assert_eq!(pages, vec![("EQ", vec![0, 1, 2]), ("DYN", vec![3, 4])]);
    }

    #[test]
    fn section_page_split_on_first_slash() {
        assert_eq!(split_group("EQ/Lo Shelf"), ("EQ", "Lo Shelf"));
        assert_eq!(split_group("EQ/Lo Shelf/Extra"), ("EQ", "Lo Shelf/Extra"));
        assert_eq!(split_group("Compressor"), ("Compressor", "Compressor"));
    }

    #[test]
    fn page_id_stable_and_distinct() {
        assert_eq!(
            remote_controls_page_id("EQ", 0),
            remote_controls_page_id("EQ", 0)
        );
        assert_ne!(
            remote_controls_page_id("EQ", 0),
            remote_controls_page_id("EQ", 1)
        );
        assert_ne!(
            remote_controls_page_id("EQ", 0),
            remote_controls_page_id("DYN", 0)
        );
    }
}

#[cfg(test)]
mod bus_layout_clap_tests {
    use aura_core::{BusLayout, ChannelConfig};

    #[test]
    fn default_layout_is_stereo() {
        // PluginLogic default is stereo; pure layout helpers used by ports.
        assert_eq!(BusLayout::stereo().main_out, ChannelConfig::Stereo);
        assert_eq!(BusLayout::mono().main_out, ChannelConfig::Mono);
        assert_eq!(BusLayout::stereo_and_mono().len(), 2);
    }
}

#[cfg(test)]
mod latency_cache_tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Mirrors the cache swap used by `Instance::update_latency_cache`.
    fn swap_latency(cache: &AtomicU32, new: u32) -> bool {
        let old = cache.swap(new, Ordering::Relaxed);
        old != new
    }

    #[test]
    fn cache_detects_change_and_stability() {
        let cache = AtomicU32::new(0);
        assert!(swap_latency(&cache, 512));
        assert!(!swap_latency(&cache, 512));
        assert!(swap_latency(&cache, 1024));
        assert_eq!(cache.load(Ordering::Relaxed), 1024);
    }
}
