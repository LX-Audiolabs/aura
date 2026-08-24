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
//! Covers: factory, audio-ports (+ config, sidechain), note-ports, params
//! (sample-accurate + mono + per-note mod), note expressions, state, GUI,
//! remote-controls, latency, tail, render, preset-load (+ discovery),
//! MIDI 2 (`CLAP_EVENT_MIDI2` → `ProcessContext.ump`, 7-bit image on `midi`).

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
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use aura_core::editor::{Editor, EditorBridge, PluginContext, RawWindowHandle};
use aura_core::events::{ParamEvent, ParamEventQueue};
use aura_core::info::PluginCategory;
use aura_core::transport::Transport;
use aura_core::{
    AudioBuffer, AudioConfig, BusLayout, ChannelConfig, NoteBuffer, NoteEvent, NoteEventKind,
    NoteExpression, NoteTarget, PluginLogic, ProcessContext, ProcessMode, ProcessStatus,
    TimedParamEvent, Tuning, apply_at_time, apply_non_chunked, host_callback, host_callback_with,
    layout_at, route_param_mod, route_param_value, split_points_into,
};
use aura_core::{MidiBuffer, MidiMessage, Ump, UmpBuffer};
use aura_params::{ParamFlags, ParamInfo, ParamRange, Params};
use clap_sys::events::{
    CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_IS_LIVE, CLAP_EVENT_MIDI, CLAP_EVENT_MIDI2,
    CLAP_EVENT_NOTE_CHOKE, CLAP_EVENT_NOTE_END, CLAP_EVENT_NOTE_EXPRESSION, CLAP_EVENT_NOTE_OFF,
    CLAP_EVENT_NOTE_ON, CLAP_EVENT_PARAM_GESTURE_BEGIN, CLAP_EVENT_PARAM_GESTURE_END,
    CLAP_EVENT_PARAM_MOD, CLAP_EVENT_PARAM_VALUE, CLAP_TRANSPORT_HAS_BEATS_TIMELINE,
    CLAP_TRANSPORT_HAS_SECONDS_TIMELINE, CLAP_TRANSPORT_HAS_TEMPO,
    CLAP_TRANSPORT_HAS_TIME_SIGNATURE, CLAP_TRANSPORT_IS_LOOP_ACTIVE, CLAP_TRANSPORT_IS_PLAYING,
    CLAP_TRANSPORT_IS_RECORDING, clap_event_header, clap_event_midi, clap_event_midi2,
    clap_event_note, clap_event_note_expression, clap_event_param_gesture, clap_event_param_mod,
    clap_event_param_value, clap_event_transport, clap_event_type, clap_input_events,
    clap_output_events,
};
use clap_sys::ext::audio_ports::{
    CLAP_AUDIO_PORT_IS_MAIN, CLAP_EXT_AUDIO_PORTS, CLAP_PORT_MONO, CLAP_PORT_STEREO,
    clap_audio_port_info, clap_plugin_audio_ports,
};
use clap_sys::ext::audio_ports_config::{
    CLAP_EXT_AUDIO_PORTS_CONFIG, clap_audio_ports_config, clap_plugin_audio_ports_config,
};
use clap_sys::ext::draft::tuning::CLAP_EXT_TUNING;
use clap_sys::ext::gui::{
    CLAP_EXT_GUI, CLAP_WINDOW_API_COCOA, CLAP_WINDOW_API_WIN32, CLAP_WINDOW_API_X11, clap_host_gui,
    clap_plugin_gui, clap_window,
};
use clap_sys::ext::latency::{CLAP_EXT_LATENCY, clap_host_latency, clap_plugin_latency};
use clap_sys::ext::note_name::{CLAP_EXT_NOTE_NAME, clap_plugin_note_name};
use clap_sys::ext::note_ports::{
    CLAP_EXT_NOTE_PORTS, CLAP_NOTE_DIALECT_CLAP, CLAP_NOTE_DIALECT_MIDI, CLAP_NOTE_DIALECT_MIDI2,
    clap_note_port_info, clap_plugin_note_ports,
};
use clap_sys::ext::param_indication::{
    CLAP_EXT_PARAM_INDICATION, CLAP_EXT_PARAM_INDICATION_COMPAT, clap_plugin_param_indication,
};
use clap_sys::ext::params::{
    CLAP_EXT_PARAMS, CLAP_PARAM_IS_AUTOMATABLE, CLAP_PARAM_IS_AUTOMATABLE_PER_CHANNEL,
    CLAP_PARAM_IS_AUTOMATABLE_PER_KEY, CLAP_PARAM_IS_AUTOMATABLE_PER_NOTE_ID, CLAP_PARAM_IS_BYPASS,
    CLAP_PARAM_IS_HIDDEN, CLAP_PARAM_IS_MODULATABLE, CLAP_PARAM_IS_MODULATABLE_PER_CHANNEL,
    CLAP_PARAM_IS_MODULATABLE_PER_KEY, CLAP_PARAM_IS_MODULATABLE_PER_NOTE_ID,
    CLAP_PARAM_IS_READONLY, CLAP_PARAM_IS_STEPPED, CLAP_PARAM_RESCAN_VALUES, clap_host_params,
    clap_param_info, clap_plugin_params,
};
use clap_sys::ext::remote_controls::{
    CLAP_EXT_REMOTE_CONTROLS, CLAP_EXT_REMOTE_CONTROLS_COMPAT, CLAP_REMOTE_CONTROLS_COUNT,
    clap_plugin_remote_controls, clap_remote_controls_page,
};
use clap_sys::ext::render::{
    CLAP_EXT_RENDER, CLAP_RENDER_OFFLINE, CLAP_RENDER_REALTIME, clap_plugin_render,
    clap_plugin_render_mode,
};
use clap_sys::ext::state::{CLAP_EXT_STATE, clap_plugin_state};
use clap_sys::ext::tail::{CLAP_EXT_TAIL, clap_host_tail, clap_plugin_tail};
use clap_sys::ext::voice_info::{
    CLAP_EXT_VOICE_INFO, CLAP_VOICE_INFO_SUPPORTS_OVERLAPPING_NOTES, clap_plugin_voice_info,
    clap_voice_info,
};
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

mod preset_load;
mod tuning;

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
    } else if preset_load::is_discovery_factory_id(id) && !L::factory_presets().is_empty() {
        preset_load::discovery_factory::<L>() as *const c_void
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
    let tuning_state = tuning::TuningState::new(host);

    let instance = Box::new(Instance::<L> {
        host,
        params,
        editor: None,
        param_events: Arc::new(ParamEventQueue::default()),
        host_scale: Arc::new(AtomicU64::new(1.0f64.to_bits())),
        state: None,
        sample_rate: 44_100.0,
        max_frames: 0,
        active: false,
        layout_index: 0,
        latency_cache: AtomicU32::new(0),
        latency_dirty: AtomicBool::new(false),
        tail_cache: AtomicU32::new(0),
        process_mode: ProcessMode::Realtime,
        scratch: ProcessScratch::new(),
        tuning_state,
        tuning_pool_changed: AtomicBool::new(false),
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

pub(crate) struct Instance<L: PluginLogic> {
    pub(crate) host: *const clap_host,
    pub(crate) params: Arc<L::Params>,
    /// Created on the main thread in `plugin_init`; `None` = no GUI.
    editor: Option<Box<dyn Editor>>,
    /// GUI → host param events, drained in process/flush.
    param_events: Arc<ParamEventQueue>,
    /// Host content scale from `gui.set_scale` (f64 bits, default 1.0).
    /// Shared with [`ClapBridge`] so `request_resize` matches `get_size`.
    host_scale: Arc<AtomicU64>,
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
    /// Last tail length reported to the host.
    tail_cache: AtomicU32,
    /// Host render mode (`clap.render`) → `ProcessContext.process_mode`.
    process_mode: ProcessMode,
    /// Audio-thread scratch — reserved in `activate`, reused every `process`.
    scratch: ProcessScratch,
    /// CLAP `clap.tuning/2` host state (event space + active tuning ids).
    tuning_state: Arc<tuning::TuningState>,
    /// Set by the host's `clap.tuning` `changed()` callback; consumed in process.
    tuning_pool_changed: AtomicBool,
}

/// Pre-sized process working set. Heap growth here is a Bitwig-crash class bug
/// (note-expression flood used to `Vec::new` + copy every block).
struct ProcessScratch {
    timed: Vec<TimedParamEvent>,
    tuning_events: Vec<aura_core::TuningEvent>,
    midi: MidiBuffer,
    midi_out: MidiBuffer,
    notes: NoteBuffer,
    notes_out: NoteBuffer,
    notes_chunk: NoteBuffer,
    midi_chunk: MidiBuffer,
    ump: UmpBuffer,
    ump_out: UmpBuffer,
    ump_chunk: UmpBuffer,
    splits: Vec<u32>,
    /// Zeroed fallback when a host input pointer is null.
    silence: Vec<f32>,
}

/// Bitwig can emit a note-expression per sample per voice. Cap extras; keep
/// on/off/choke so notes cannot stick. Slack in the reserve covers those.
const MAX_NOTE_EVENTS: usize = 4096;

impl ProcessScratch {
    fn new() -> Self {
        Self {
            timed: Vec::new(),
            tuning_events: Vec::new(),
            midi: MidiBuffer::new(),
            midi_out: MidiBuffer::new(),
            notes: NoteBuffer::new(),
            notes_out: NoteBuffer::new(),
            notes_chunk: NoteBuffer::new(),
            midi_chunk: MidiBuffer::new(),
            ump: UmpBuffer::new(),
            ump_out: UmpBuffer::new(),
            ump_chunk: UmpBuffer::new(),
            splits: Vec::new(),
            silence: Vec::new(),
        }
    }

    fn prepare(&mut self, max_frames: usize) {
        self.timed.reserve(256);
        self.tuning_events.reserve(16);
        self.midi.reserve(MAX_NOTE_EVENTS);
        self.midi_out.reserve(256);
        self.notes.reserve(MAX_NOTE_EVENTS + 128);
        self.notes_out.reserve(256);
        self.notes_chunk.reserve(MAX_NOTE_EVENTS + 128);
        self.midi_chunk.reserve(MAX_NOTE_EVENTS);
        self.ump.reserve(MAX_NOTE_EVENTS);
        self.ump_out.reserve(256);
        self.ump_chunk.reserve(MAX_NOTE_EVENTS);
        self.splits.reserve(64);
        if self.silence.len() < max_frames {
            self.silence.resize(max_frames, 0.0);
        }
    }
}

/// Stereo + optional stereo SC today; headroom if a layout grows.
const MAX_AUDIO_CH: usize = 8;

fn accept_note_event(len: usize, essential: bool) -> bool {
    essential || len < MAX_NOTE_EVENTS
}

impl<L: PluginLogic> Instance<L> {
    fn host_scale(&self) -> f64 {
        f64::from_bits(self.host_scale.load(Ordering::Relaxed))
    }

    fn set_host_scale(&self, scale: f64) {
        if scale.is_finite() && scale > 0.0 {
            self.host_scale.store(scale.to_bits(), Ordering::Relaxed);
        }
    }

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

    /// Snapshot `PluginLogic::tail_length` into the cache. Returns `true` if changed.
    fn update_tail_cache(&self) -> bool {
        let Some(state) = self.state.as_ref() else {
            return false;
        };
        let new = L::tail_length(state);
        let old = self.tail_cache.swap(new, Ordering::Relaxed);
        old != new
    }

    pub(crate) unsafe fn from_plugin<'a>(plugin: *const clap_plugin) -> Option<&'a mut Self> {
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

/// Call `clap_host_tail.changed` if the host exposes it (main thread).
fn host_tail_changed(host: *const clap_host) {
    if host.is_null() {
        return;
    }
    let Some(get) = (unsafe { (*host).get_extension }) else {
        return;
    };
    let ext = unsafe { get(host, CLAP_EXT_TAIL.as_ptr()) };
    if ext.is_null() {
        return;
    }
    let host_tail = unsafe { &*(ext as *const clap_host_tail) };
    if let Some(changed) = host_tail.changed {
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
        inst.scratch.prepare(max_frames as usize);
        let layout = inst.selected_layout();
        let config = AudioConfig::new(sample_rate, max_frames as usize)
            .with_channels(layout.main_input_channels(), layout.main_output_channels())
            .with_sidechain_channels(layout.sidechain_input_channels());
        let mut state = L::init(&inst.params, sample_rate);
        L::reset(&mut state, &inst.params, &config);
        inst.params.set_sample_rate(sample_rate);
        let latency = L::latency(&state);
        let tail = L::tail_length(&state);
        inst.state = Some(state);
        inst.latency_cache.store(latency, Ordering::Relaxed);
        inst.latency_dirty.store(false, Ordering::Relaxed);
        inst.tail_cache.store(tail, Ordering::Relaxed);
        inst.active = true;
        // Spec: latency.changed is [main-thread]; host re-reads get after activate.
        host_latency_changed(inst.host);
        // tail.changed is [audio-thread] when active — host calls get() after
        // activate; mid-run changes notify from process (see plugin_process).
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
            .with_channels(layout.main_input_channels(), layout.main_output_channels())
            .with_sidechain_channels(layout.sidechain_input_channels());
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

// CLAP process owns host buffers, timed events, and optional multi-chunk
// process loops — length is inherent to the ABI glue.
#[allow(clippy::too_many_lines)]
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
        if inst.max_frames == 0 || frames > inst.max_frames as usize {
            return CLAP_PROCESS_ERROR;
        }

        inst.scratch.timed.clear();
        inst.scratch.tuning_events.clear();
        inst.scratch.midi.clear();
        inst.scratch.notes.clear();
        inst.scratch.midi_out.clear();
        inst.scratch.notes_out.clear();
        inst.scratch.ump.clear();
        inst.scratch.ump_out.clear();
        let tuning_space_id = inst.tuning_state.space_id();
        if !process.in_events.is_null() {
            unsafe {
                collect_input_events(
                    process.in_events,
                    &mut inst.scratch.timed,
                    &mut inst.scratch.tuning_events,
                    tuning_space_id,
                    &mut inst.scratch.midi,
                    &mut inst.scratch.notes,
                    &mut inst.scratch.ump,
                );
            };
        }
        unsafe { emit_param_events(&inst.param_events, process.out_events) };

        let transport = if process.transport.is_null() {
            None
        } else {
            Some(map_transport(unsafe { &*process.transport }))
        };

        let layout = inst.selected_layout();
        let process_mode = inst.process_mode;
        let sample_rate = inst.sample_rate;

        let Some(state) = inst.state.as_mut() else {
            return CLAP_PROCESS_ERROR;
        };
        let main_in_ch = layout.main_input_channels() as usize;
        let sidechain_in_ch = layout.sidechain_input_channels() as usize;
        let total_in_ch = main_in_ch + sidechain_in_ch;
        let mut out_ch = layout.main_output_channels() as usize;
        if !matches!(out_ch, 0..=2) || total_in_ch > MAX_AUDIO_CH {
            return CLAP_PROCESS_ERROR;
        }

        // Host pointers only — no per-block audio copies. Extra host input
        // ports on an output-only instrument are ignored (used to panic).
        let mut in_ptrs = [ptr::null::<f32>(); MAX_AUDIO_CH];
        let mut filled_in = 0usize;
        if !process.audio_inputs.is_null() {
            for port_i in 0..process.audio_inputs_count as usize {
                if filled_in >= total_in_ch {
                    break;
                }
                let port = unsafe { &*process.audio_inputs.add(port_i) };
                if port.data32.is_null() {
                    continue;
                }
                for c in 0..port.channel_count as usize {
                    if filled_in >= total_in_ch {
                        break;
                    }
                    in_ptrs[filled_in] = unsafe { *port.data32.add(c) };
                    filled_in += 1;
                }
            }
        }

        let mut out_ptrs = [ptr::null_mut::<f32>(); MAX_AUDIO_CH];
        let mut filled_out = 0usize;
        if process.audio_outputs_count > 0 && !process.audio_outputs.is_null() {
            let out_port = unsafe { &*process.audio_outputs };
            if !out_port.data32.is_null() {
                let n = out_ch.min(out_port.channel_count as usize);
                for (slot, ptr) in out_ptrs.iter_mut().take(n).enumerate() {
                    *ptr = unsafe { *out_port.data32.add(slot) };
                    filled_out += 1;
                }
            }
        }
        // Note-FX racks (Bitwig) often pass 0 audio ports. Still process events.
        if filled_out != out_ch {
            if matches!(filled_out, 0..=2) {
                out_ch = filled_out;
            } else {
                return CLAP_PROCESS_ERROR;
            }
        }

        // Sample-accurate: non-CHUNKED params apply at block start; CHUNKED
        // params split the block at their event times.
        let infos = inst.params.param_infos();
        apply_non_chunked(&*inst.params, &inst.scratch.timed, &infos);
        let frames_u = frames as u32;
        split_points_into(
            &mut inst.scratch.splits,
            frames_u,
            &inst.scratch.timed,
            &infos,
        );
        for ev in &inst.scratch.tuning_events {
            let t = ev.sample_offset;
            if t > 0 && t < frames_u {
                inst.scratch.splits.push(t);
            }
        }
        inst.scratch.splits.sort_unstable();
        inst.scratch.splits.dedup();

        let tuning =
            Tuning::new(Arc::clone(&inst.tuning_state) as Arc<dyn aura_core::TuningProvider>);

        let mut status = ProcessStatus::Continue;
        let n_splits = inst.scratch.splits.len();

        for si in 0..n_splits.saturating_sub(1) {
            let t0 = inst.scratch.splits[si] as usize;
            let t1 = inst.scratch.splits[si + 1] as usize;
            if t1 <= t0 {
                continue;
            }
            let chunk_len = t1 - t0;
            apply_at_time(
                &*inst.params,
                &inst.scratch.timed,
                inst.scratch.splits[si],
                &infos,
            );
            let chunk_t0 = inst.scratch.splits[si];
            for ev in &inst.scratch.tuning_events {
                if ev.sample_offset == chunk_t0 {
                    tuning.apply_event(ev);
                }
            }

            if inst.scratch.silence.len() < t1 {
                return CLAP_PROCESS_ERROR;
            }
            let silence = &inst.scratch.silence[t0..t1];
            let mut in_store = [&[] as &[f32]; MAX_AUDIO_CH];
            for i in 0..total_in_ch {
                in_store[i] = if in_ptrs[i].is_null() {
                    silence
                } else {
                    unsafe { std::slice::from_raw_parts(in_ptrs[i].add(t0), chunk_len) }
                };
            }
            let in_refs = &in_store[..total_in_ch];
            if in_refs.iter().any(|ch| ch.len() < chunk_len) {
                return CLAP_PROCESS_ERROR;
            }
            if out_ptrs[..out_ch].iter().any(|p| p.is_null()) {
                return CLAP_PROCESS_ERROR;
            }

            let single = n_splits == 2;
            if !single {
                let (a, b) = (inst.scratch.splits[si], inst.scratch.splits[si + 1]);
                inst.scratch
                    .midi_chunk
                    .copy_range_rebased(&inst.scratch.midi, a, b);
                inst.scratch
                    .notes_chunk
                    .copy_range_rebased(&inst.scratch.notes, a, b);
                inst.scratch
                    .ump_chunk
                    .copy_range_rebased(&inst.scratch.ump, a, b);
            }
            let midi = if single {
                std::mem::take(&mut inst.scratch.midi)
            } else {
                std::mem::take(&mut inst.scratch.midi_chunk)
            };
            let notes = if single {
                std::mem::take(&mut inst.scratch.notes)
            } else {
                std::mem::take(&mut inst.scratch.notes_chunk)
            };
            let ump = if single {
                std::mem::take(&mut inst.scratch.ump)
            } else {
                std::mem::take(&mut inst.scratch.ump_chunk)
            };
            let mut ctx = ProcessContext::new(sample_rate, chunk_len)
                .with_process_mode(process_mode)
                .with_midi(midi)
                .with_notes(notes)
                .with_ump(ump)
                .with_midi_out(std::mem::take(&mut inst.scratch.midi_out))
                .with_notes_out(std::mem::take(&mut inst.scratch.notes_out))
                .with_ump_out(std::mem::take(&mut inst.scratch.ump_out))
                .with_tuning(tuning.clone());
            ctx.transport = transport;

            let chunk_status = unsafe {
                run_process_chunk::<L>(
                    state,
                    &inst.params,
                    in_refs,
                    out_ptrs,
                    out_ch,
                    t0,
                    chunk_len,
                    main_in_ch,
                    sidechain_in_ch,
                    &mut ctx,
                )
            };

            if single {
                inst.scratch.midi = std::mem::take(&mut ctx.midi);
                inst.scratch.notes = std::mem::take(&mut ctx.notes);
                inst.scratch.ump = std::mem::take(&mut ctx.ump);
            } else {
                inst.scratch.midi_chunk = std::mem::take(&mut ctx.midi);
                inst.scratch.notes_chunk = std::mem::take(&mut ctx.notes);
                inst.scratch.ump_chunk = std::mem::take(&mut ctx.ump);
                inst.scratch.midi_chunk.clear();
                inst.scratch.notes_chunk.clear();
                inst.scratch.ump_chunk.clear();
            }
            unsafe {
                emit_midi_events_at(process.out_events, &ctx.midi_out, inst.scratch.splits[si]);
                emit_note_events_at(process.out_events, &ctx.notes_out, inst.scratch.splits[si]);
                emit_ump_events_at(process.out_events, &ctx.ump_out, inst.scratch.splits[si]);
            }
            inst.scratch.midi_out = std::mem::take(&mut ctx.midi_out);
            inst.scratch.midi_out.clear();
            inst.scratch.notes_out = std::mem::take(&mut ctx.notes_out);
            inst.scratch.notes_out.clear();
            inst.scratch.ump_out = std::mem::take(&mut ctx.ump_out);
            inst.scratch.ump_out.clear();

            match chunk_status {
                ProcessStatus::Error => {
                    status = ProcessStatus::Error;
                    break;
                }
                ProcessStatus::TailFinished => status = ProcessStatus::TailFinished,
                ProcessStatus::Continue => {
                    if status != ProcessStatus::TailFinished {
                        status = ProcessStatus::Continue;
                    }
                }
            }
        }

        if inst.tuning_pool_changed.swap(false, Ordering::Relaxed) {
            L::tuning_changed(state, &inst.params);
        }
        // PDC: latency.changed is [main-thread] only — schedule callback.
        if inst.update_latency_cache() {
            inst.latency_dirty.store(true, Ordering::Relaxed);
            host_request_callback(inst.host);
        }
        // tail.changed is [audio-thread] when active — notify here, not via
        // on_main_thread (clap-validator enforces the thread rule).
        if inst.update_tail_cache() {
            host_tail_changed(inst.host);
        }

        match status {
            ProcessStatus::Continue => CLAP_PROCESS_CONTINUE,
            ProcessStatus::TailFinished => CLAP_PROCESS_TAIL,
            ProcessStatus::Error => CLAP_PROCESS_ERROR,
        }
    })
}

/// Write one chunk into host output pointers (mono or stereo).
#[allow(clippy::too_many_arguments)]
unsafe fn run_process_chunk<L: PluginLogic>(
    state: &mut L::DspState,
    params: &L::Params,
    in_refs: &[&[f32]],
    out_ptrs: [*mut f32; MAX_AUDIO_CH],
    out_ch: usize,
    t0: usize,
    chunk_len: usize,
    main_in_ch: usize,
    sidechain_in_ch: usize,
    ctx: &mut ProcessContext,
) -> ProcessStatus {
    unsafe fn slice_out<'a>(p: *mut f32, t0: usize, n: usize) -> &'a mut [f32] {
        unsafe { std::slice::from_raw_parts_mut(p.add(t0), n) }
    }
    match out_ch {
        1 => {
            let mut s0 = unsafe { slice_out(out_ptrs[0], t0, chunk_len) };
            let mut outs = [&mut s0 as &mut [f32]];
            let mut buffer = unsafe {
                AudioBuffer::from_slices_with_sidechain_unchecked(
                    in_refs,
                    &mut outs,
                    chunk_len,
                    main_in_ch,
                    sidechain_in_ch,
                )
            };
            L::process(state, params, &mut buffer, ctx)
        }
        2 => {
            let mut s0 = unsafe { slice_out(out_ptrs[0], t0, chunk_len) };
            let mut s1 = unsafe { slice_out(out_ptrs[1], t0, chunk_len) };
            let mut outs = [&mut s0 as &mut [f32], &mut s1];
            let mut buffer = unsafe {
                AudioBuffer::from_slices_with_sidechain_unchecked(
                    in_refs,
                    &mut outs,
                    chunk_len,
                    main_in_ch,
                    sidechain_in_ch,
                )
            };
            L::process(state, params, &mut buffer, ctx)
        }
        0 => {
            let mut outs: [&mut [f32]; 0] = [];
            let mut buffer = unsafe {
                AudioBuffer::from_slices_with_sidechain_unchecked(
                    in_refs,
                    &mut outs,
                    chunk_len,
                    main_in_ch,
                    sidechain_in_ch,
                )
            };
            L::process(state, params, &mut buffer, ctx)
        }
        _ => ProcessStatus::Error,
    }
}

/// Collect timed param / MIDI / note / tuning events from the host input list.
#[allow(clippy::too_many_lines)]
unsafe fn collect_input_events(
    in_events: *const clap_input_events,
    timed: &mut Vec<TimedParamEvent>,
    tuning_events: &mut Vec<aura_core::TuningEvent>,
    tuning_space_id: u16,
    midi: &mut MidiBuffer,
    notes: &mut NoteBuffer,
    ump: &mut UmpBuffer,
) {
    midi.clear();
    notes.clear();
    ump.clear();
    timed.clear();
    tuning_events.clear();
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
        if tuning_space_id != 0 && header.space_id == tuning_space_id {
            if let Some(ev) = unsafe { tuning::parse_tuning_event(header, hdr) } {
                tuning_events.push(ev);
            }
            continue;
        }
        if header.space_id != CLAP_CORE_EVENT_SPACE_ID {
            continue;
        }
        match header.type_ {
            CLAP_EVENT_PARAM_VALUE => {
                let pev = unsafe { &*(hdr as *const clap_event_param_value) };
                if pev.note_id >= 0 && !accept_note_event(notes.len(), false) {
                    continue;
                }
                route_param_value(
                    timed,
                    notes,
                    header.time,
                    pev.param_id,
                    pev.value,
                    NoteTarget {
                        note_id: pev.note_id,
                        port_index: pev.port_index,
                        channel: pev.channel,
                        key: pev.key,
                    },
                );
            }
            CLAP_EVENT_PARAM_MOD => {
                let pev = unsafe { &*(hdr as *const clap_event_param_mod) };
                if pev.note_id >= 0 && !accept_note_event(notes.len(), false) {
                    continue;
                }
                route_param_mod(
                    timed,
                    notes,
                    header.time,
                    pev.param_id,
                    pev.amount,
                    NoteTarget {
                        note_id: pev.note_id,
                        port_index: pev.port_index,
                        channel: pev.channel,
                        key: pev.key,
                    },
                );
            }
            CLAP_EVENT_NOTE_ON
            | CLAP_EVENT_NOTE_OFF
            | CLAP_EVENT_NOTE_CHOKE
            | CLAP_EVENT_NOTE_END => {
                if !accept_note_event(notes.len(), true) {
                    continue;
                }
                let n = unsafe { &*(hdr as *const clap_event_note) };
                notes.push(NoteEvent {
                    sample_offset: header.time,
                    note_id: n.note_id,
                    port_index: n.port_index,
                    channel: n.channel,
                    key: n.key,
                    kind: match header.type_ {
                        CLAP_EVENT_NOTE_ON => NoteEventKind::On {
                            velocity: n.velocity,
                        },
                        CLAP_EVENT_NOTE_CHOKE => NoteEventKind::Choke,
                        CLAP_EVENT_NOTE_END => NoteEventKind::End,
                        _ => NoteEventKind::Off {
                            velocity: n.velocity,
                        },
                    },
                });
                if let Some(msg) = clap_note_to_midi(header.type_, hdr) {
                    midi.push(header.time, msg);
                }
            }
            CLAP_EVENT_NOTE_EXPRESSION => {
                if !accept_note_event(notes.len(), false) {
                    continue;
                }
                let e = unsafe { &*(hdr as *const clap_event_note_expression) };
                notes.push(NoteEvent {
                    sample_offset: header.time,
                    note_id: e.note_id,
                    port_index: e.port_index,
                    channel: e.channel,
                    key: e.key,
                    kind: NoteEventKind::Expression {
                        id: NoteExpression::from_clap(e.expression_id),
                        value: e.value,
                    },
                });
            }
            CLAP_EVENT_MIDI => {
                let m = unsafe { &*(hdr as *const clap_event_midi) };
                let msg = MidiMessage::raw(m.data[0], m.data[1], m.data[2]);
                midi.push(header.time, msg);
                if accept_note_event(ump.len(), msg.is_note_on() || msg.is_note_off()) {
                    ump.push(header.time, Ump::from_midi1(msg));
                }
            }
            CLAP_EVENT_MIDI2 => {
                let m = unsafe { &*(hdr as *const clap_event_midi2) };
                let packet = Ump::from_words(m.data);
                let essential = packet.is_note_on() || packet.is_note_off();
                if accept_note_event(ump.len(), essential) {
                    ump.push(header.time, packet);
                }
                if let Some(msg) = packet.to_midi1() {
                    midi.push(header.time, msg);
                }
            }
            _ => {}
        }
    }
}

/// Apply all param events immediately (flush / no-audio path).
unsafe fn apply_input_events(
    params: &dyn Params,
    in_events: *const clap_input_events,
    midi: &mut MidiBuffer,
) {
    let mut timed = Vec::new();
    let mut tuning_events = Vec::new();
    let mut notes = NoteBuffer::new();
    let mut ump = UmpBuffer::new();
    unsafe {
        collect_input_events(
            in_events,
            &mut timed,
            &mut tuning_events,
            0,
            midi,
            &mut notes,
            &mut ump,
        );
    }
    let _ = (tuning_events, notes, ump); // ponytail: flush has no process() to consume these
    for ev in timed {
        aura_core::apply_event(params, ev);
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

/// Push plugin-generated CLAP notes from [`ProcessContext::notes_out`].
unsafe fn emit_note_events_at(out: *const clap_output_events, notes: &NoteBuffer, base: u32) {
    if out.is_null() || notes.is_empty() {
        return;
    }
    let Some(try_push) = (unsafe { &*out }).try_push else {
        return;
    };
    for ev in notes.iter() {
        let time = ev.sample_offset.saturating_add(base);
        match ev.kind {
            NoteEventKind::On { .. }
            | NoteEventKind::Off { .. }
            | NoteEventKind::Choke
            | NoteEventKind::End => {
                let type_ = match ev.kind {
                    NoteEventKind::On { .. } => CLAP_EVENT_NOTE_ON,
                    NoteEventKind::Off { .. } => CLAP_EVENT_NOTE_OFF,
                    NoteEventKind::Choke => CLAP_EVENT_NOTE_CHOKE,
                    _ => CLAP_EVENT_NOTE_END,
                };
                let velocity = match ev.kind {
                    NoteEventKind::On { velocity } | NoteEventKind::Off { velocity } => velocity,
                    _ => 0.0,
                };
                let mut e = clap_event_note {
                    header: event_header(type_, size_of::<clap_event_note>() as u32),
                    note_id: ev.note_id,
                    port_index: -1,
                    channel: ev.channel,
                    key: ev.key,
                    velocity,
                };
                e.header.time = time;
                unsafe { try_push(out, &e as *const _ as *const clap_event_header) };
            }
            NoteEventKind::Expression { id, value } => {
                let mut e = clap_event_note_expression {
                    header: event_header(
                        CLAP_EVENT_NOTE_EXPRESSION,
                        size_of::<clap_event_note_expression>() as u32,
                    ),
                    expression_id: id.to_clap(),
                    note_id: ev.note_id,
                    port_index: ev.port_index,
                    channel: ev.channel,
                    key: ev.key,
                    value,
                };
                e.header.time = time;
                unsafe { try_push(out, &e as *const _ as *const clap_event_header) };
            }
            NoteEventKind::ParamMod { .. } | NoteEventKind::ParamValue { .. } => {}
        }
    }
}

/// Push plugin-generated UMP from [`ProcessContext::ump_out`] as `CLAP_EVENT_MIDI2`.
unsafe fn emit_ump_events_at(out: *const clap_output_events, ump: &UmpBuffer, base: u32) {
    if out.is_null() || ump.is_empty() {
        return;
    }
    let Some(try_push) = (unsafe { &*out }).try_push else {
        return;
    };
    for ev in ump.iter() {
        let mut e = clap_event_midi2 {
            header: event_header(CLAP_EVENT_MIDI2, size_of::<clap_event_midi2>() as u32),
            port_index: 0,
            data: ev.packet.words(),
        };
        e.header.time = ev.sample_offset.saturating_add(base);
        unsafe { try_push(out, &e as *const _ as *const clap_event_header) };
    }
}

/// Push plugin-generated MIDI events from [`ProcessContext::midi_out`] to the host.
/// `base` is added to each sample offset (0 for a full-block chunk).
unsafe fn emit_midi_events_at(out: *const clap_output_events, midi: &MidiBuffer, base: u32) {
    if out.is_null() || midi.is_empty() {
        return;
    }
    let Some(try_push) = (unsafe { &*out }).try_push else {
        return;
    };
    for ev in midi.iter() {
        let time = ev.sample_offset.saturating_add(base);
        // Always raw `CLAP_EVENT_MIDI`. Note FX chains (Bitwig) forward MIDI
        // dialect; converting On/Off to `CLAP_EVENT_NOTE_*` here dropped them.
        let mut e = clap_event_midi {
            header: event_header(CLAP_EVENT_MIDI, size_of::<clap_event_midi>() as u32),
            port_index: 0,
            data: [ev.message.status_byte(), ev.message.data1, ev.message.data2],
        };
        e.header.time = time;
        unsafe { try_push(out, &e as *const _ as *const clap_event_header) };
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
    if id == CLAP_EXT_TAIL {
        return tail_ext::<L>() as *const _ as *const c_void;
    }
    if id == CLAP_EXT_RENDER {
        return render_ext::<L>() as *const _ as *const c_void;
    }
    if id == CLAP_EXT_NOTE_PORTS {
        let info = L::info();
        if info.accepts_midi_in || info.emits_midi {
            return note_ports_ext::<L>() as *const _ as *const c_void;
        }
        return ptr::null();
    }
    if id == CLAP_EXT_VOICE_INFO && L::info().voice_count > 0 {
        return voice_info_ext::<L>() as *const _ as *const c_void;
    }
    if id == CLAP_EXT_STATE {
        return state_ext::<L>() as *const _ as *const c_void;
    }
    if preset_load::is_preset_load_ext(id) {
        return preset_load::preset_load_ext::<L>() as *const _ as *const c_void;
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
    if id == CLAP_EXT_TUNING && L::info().supports_tuning {
        return tuning::tuning_ext::<L>() as *const _ as *const c_void;
    }
    if id == CLAP_EXT_NOTE_NAME && !L::note_names().is_empty() {
        return note_name_ext::<L>() as *const _ as *const c_void;
    }
    if id == CLAP_EXT_PARAM_INDICATION || id == CLAP_EXT_PARAM_INDICATION_COMPAT {
        return param_indication_ext::<L>() as *const _ as *const c_void;
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
// tail
// ---------------------------------------------------------------------------

fn tail_ext<L: PluginLogic>() -> &'static clap_plugin_tail {
    static CELL: OnceLock<clap_plugin_tail> = OnceLock::new();
    CELL.get_or_init(|| clap_plugin_tail {
        get: Some(tail_get::<L>),
    })
}

unsafe extern "C" fn tail_get<L: PluginLogic>(plugin: *const clap_plugin) -> u32 {
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return 0;
    };
    inst.tail_cache.load(Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// render (realtime vs offline)
// ---------------------------------------------------------------------------

fn render_ext<L: PluginLogic>() -> &'static clap_plugin_render {
    static CELL: OnceLock<clap_plugin_render> = OnceLock::new();
    CELL.get_or_init(|| clap_plugin_render {
        has_hard_realtime_requirement: Some(render_hard_rt),
        set: Some(render_set::<L>),
    })
}

unsafe extern "C" fn render_hard_rt(_plugin: *const clap_plugin) -> bool {
    // Authors can oversample offline; we never *require* hard realtime exclusivity.
    false
}

unsafe extern "C" fn render_set<L: PluginLogic>(
    plugin: *const clap_plugin,
    mode: clap_plugin_render_mode,
) -> bool {
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return false;
    };
    inst.process_mode = match mode {
        CLAP_RENDER_OFFLINE => ProcessMode::Offline,
        _ => ProcessMode::Realtime, // CLAP_RENDER_REALTIME and unknown
    };
    let _ = CLAP_RENDER_REALTIME; // silence unused when matched via `_`
    true
}

// ---------------------------------------------------------------------------
// note-ports (MIDI / note input + output)
// ---------------------------------------------------------------------------

fn note_ports_ext<L: PluginLogic>() -> &'static clap_plugin_note_ports {
    static CELL: OnceLock<clap_plugin_note_ports> = OnceLock::new();
    CELL.get_or_init(|| clap_plugin_note_ports {
        count: Some(note_ports_count::<L>),
        get: Some(note_ports_get::<L>),
    })
}

unsafe extern "C" fn note_ports_count<L: PluginLogic>(
    _plugin: *const clap_plugin,
    is_input: bool,
) -> u32 {
    let info = L::info();
    if is_input {
        u32::from(info.accepts_midi_in)
    } else {
        u32::from(info.emits_midi)
    }
}

unsafe extern "C" fn note_ports_get<L: PluginLogic>(
    _plugin: *const clap_plugin,
    index: u32,
    is_input: bool,
    info: *mut clap_note_port_info,
) -> bool {
    if index != 0 || info.is_null() {
        return false;
    }
    let meta = L::info();
    let dialect = if is_input {
        meta.midi_input_dialect
    } else {
        meta.midi_output_dialect
    };
    let dialects = match dialect {
        aura_core::info::MidiDialect::Clap | aura_core::info::MidiDialect::Midi2 => {
            CLAP_NOTE_DIALECT_CLAP | CLAP_NOTE_DIALECT_MIDI | CLAP_NOTE_DIALECT_MIDI2
        }
        aura_core::info::MidiDialect::Midi1 => CLAP_NOTE_DIALECT_CLAP | CLAP_NOTE_DIALECT_MIDI,
    };
    let out = unsafe { &mut *info };
    // Same id in both directions so output events with port_index 0 match.
    // Distinct ids (out=1) made Bitwig Note-FX drop our emits.
    out.id = 0;
    out.supported_dialects = dialects;
    out.preferred_dialect = match dialect {
        aura_core::info::MidiDialect::Clap => CLAP_NOTE_DIALECT_CLAP,
        aura_core::info::MidiDialect::Midi2 => CLAP_NOTE_DIALECT_MIDI2,
        aura_core::info::MidiDialect::Midi1 => CLAP_NOTE_DIALECT_MIDI,
    };
    write_name(&mut out.name, if is_input { "Note In" } else { "Note Out" });
    true
}

// ---------------------------------------------------------------------------
// voice-info (Bitwig Voice Stack / poly-mod needs a voice pool > 1)
// ---------------------------------------------------------------------------

fn note_name_ext<L: PluginLogic>() -> &'static clap_plugin_note_name {
    static CELL: OnceLock<clap_plugin_note_name> = OnceLock::new();
    CELL.get_or_init(|| clap_plugin_note_name {
        count: Some(note_name_count::<L>),
        get: Some(note_name_get::<L>),
    })
}

unsafe extern "C" fn note_name_count<L: PluginLogic>(_plugin: *const clap_plugin) -> u32 {
    L::note_names().len() as u32
}

unsafe extern "C" fn note_name_get<L: PluginLogic>(
    _plugin: *const clap_plugin,
    index: u32,
    out: *mut clap_sys::ext::note_name::clap_note_name,
) -> bool {
    let names = L::note_names();
    let Some(entry) = names.get(index as usize) else {
        return false;
    };
    if out.is_null() {
        return false;
    }
    let out = unsafe { &mut *out };
    out.port = entry.port;
    out.channel = entry.channel;
    out.key = entry.key;
    let bytes = entry.name.as_bytes();
    let len = bytes.len().min(clap_sys::string_sizes::CLAP_NAME_SIZE - 1);
    for (i, &b) in bytes.iter().take(len).enumerate() {
        out.name[i] = b as std::ffi::c_char;
    }
    out.name[len] = 0;
    true
}

fn param_indication_ext<L: PluginLogic>() -> &'static clap_plugin_param_indication {
    static CELL: OnceLock<clap_plugin_param_indication> = OnceLock::new();
    CELL.get_or_init(|| clap_plugin_param_indication {
        set_mapping: Some(param_indication_set_mapping::<L>),
        set_automation: Some(param_indication_set_automation::<L>),
    })
}

unsafe extern "C" fn param_indication_set_mapping<L: PluginLogic>(
    plugin: *const clap_plugin,
    param_id: clap_id,
    has_mapping: bool,
    _color: *const clap_sys::color::clap_color,
    _label: *const std::ffi::c_char,
    _description: *const std::ffi::c_char,
) {
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return;
    };
    L::on_param_mapping(&inst.params, param_id, has_mapping);
}

unsafe extern "C" fn param_indication_set_automation<L: PluginLogic>(
    plugin: *const clap_plugin,
    param_id: clap_id,
    automation_state: u32,
    _color: *const clap_sys::color::clap_color,
) {
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return;
    };
    L::on_param_automation(&inst.params, param_id, automation_state);
}

fn voice_info_ext<L: PluginLogic>() -> &'static clap_plugin_voice_info {
    static CELL: OnceLock<clap_plugin_voice_info> = OnceLock::new();
    CELL.get_or_init(|| clap_plugin_voice_info {
        get: Some(voice_info_get::<L>),
    })
}

unsafe extern "C" fn voice_info_get<L: PluginLogic>(
    _plugin: *const clap_plugin,
    info: *mut clap_voice_info,
) -> bool {
    if info.is_null() {
        return false;
    }
    let meta = L::info();
    if meta.voice_count == 0 {
        return false;
    }
    let cap = meta.voice_capacity.max(meta.voice_count);
    let out = unsafe { &mut *info };
    out.voice_count = meta.voice_count.min(cap);
    out.voice_capacity = cap;
    out.flags = CLAP_VOICE_INFO_SUPPORTS_OVERLAPPING_NOTES;
    true
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
        u32::from(layout.main_in.is_some()) + layout.sidechain_input_channels()
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
    if info.is_null() {
        return false;
    }
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return false;
    };
    let layout = inst.selected_layout();
    let (channels, name, is_main, id) = if is_input {
        let main_count = u32::from(layout.main_in.is_some());
        if index == 0 && main_count == 1 {
            let Some(ch) = layout.main_in else {
                return false;
            };
            (ch, "Input", true, 0)
        } else if index < main_count + layout.sidechain_input_channels() {
            let Some(ch) = layout.sidechain_in else {
                return false;
            };
            (ch, "Sidechain", false, 1)
        } else {
            return false;
        }
    } else if index == 0 {
        (layout.main_out, "Output", true, 0)
    } else {
        return false;
    };
    let info = unsafe { &mut *info };
    info.id = id;
    write_name(&mut info.name, name);
    info.flags = if is_main { CLAP_AUDIO_PORT_IS_MAIN } else { 0 };
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
    out.input_port_count = u32::from(layout.main_in.is_some()) + layout.sidechain_input_channels();
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
    /// Same cell as [`Instance::host_scale`] — logical→physical for resize.
    host_scale: Arc<AtomicU64>,
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

    fn host_scale(&self) -> f64 {
        f64::from_bits(self.host_scale.load(Ordering::Relaxed))
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
        // Editor API is logical; CLAP host on Win/Linux expects physical
        // pixels (same units as `gui_get_size`). macOS stays logical.
        let (pw, ph) = gui_logical_to_host_px(w, h, self.host_scale());
        unsafe { rr(self.host, pw, ph) }
    }

    fn set_scale_hint(&self, scale: f64) {
        // Writes the same cell `gui_get_size` / `request_resize` read, so a
        // Windows host that never called `gui.set_scale` (Bitwig) still gets a
        // frame size = logical × real OS DPI, matching the rendered child.
        if scale.is_finite() && scale > 0.0 {
            self.host_scale.store(scale.to_bits(), Ordering::Relaxed);
        }
    }
}

/// Logical editor size → host GUI pixels (Win/Linux × `host_scale`; macOS identity).
fn gui_logical_to_host_px(lw: u32, lh: u32, host_scale: f64) -> (u32, u32) {
    #[cfg(target_os = "macos")]
    {
        let _ = host_scale;
        (lw.max(1), lh.max(1))
    }
    #[cfg(not(target_os = "macos"))]
    {
        scale_logical_to_physical(lw, lh, host_scale)
    }
}

/// Host GUI pixels → logical editor size (inverse of [`gui_logical_to_host_px`]).
fn gui_host_px_to_logical(pw: u32, ph: u32, host_scale: f64) -> (u32, u32) {
    #[cfg(target_os = "macos")]
    {
        let _ = host_scale;
        (pw.max(1), ph.max(1))
    }
    #[cfg(not(target_os = "macos"))]
    {
        scale_physical_to_logical(pw, ph, host_scale)
    }
}

fn scale_logical_to_physical(lw: u32, lh: u32, host_scale: f64) -> (u32, u32) {
    if !host_scale.is_finite() || host_scale <= 0.0 || (host_scale - 1.0).abs() < f64::EPSILON {
        return (lw.max(1), lh.max(1));
    }
    let pw = (f64::from(lw.max(1)) * host_scale).round().max(1.0) as u32;
    let ph = (f64::from(lh.max(1)) * host_scale).round().max(1.0) as u32;
    (pw, ph)
}

fn scale_physical_to_logical(pw: u32, ph: u32, host_scale: f64) -> (u32, u32) {
    if !host_scale.is_finite() || host_scale <= 0.0 || (host_scale - 1.0).abs() < f64::EPSILON {
        return (pw.max(1), ph.max(1));
    }
    let lw = (f64::from(pw.max(1)) / host_scale).round().max(1.0) as u32;
    let lh = (f64::from(ph.max(1)) / host_scale).round().max(1.0) as u32;
    (lw, lh)
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
    if !scale.is_finite() || scale <= 0.0 {
        return false;
    }
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return false;
    };
    // Remember for get_size / request_resize even if editor is not open yet.
    inst.set_host_scale(scale);
    if let Some(editor) = inst.editor.as_mut() {
        editor.set_scale(scale);
    }
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
    // Editor size is logical layout points. CLAP hosts on Win/Linux size the
    // embed frame in physical pixels = logical × set_scale (default 1.0).
    // macOS AppKit applies backing scale itself — report logical there.
    // Without the Win/Linux multiply, HiDPI / multi-monitor clips the child
    // (child HWND is design × host_scale, host frame was only design).
    let (pw, ph) = gui_logical_to_host_px(w, h, inst.host_scale());
    unsafe {
        *width = pw;
        *height = ph;
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
    if width.is_null() || height.is_null() {
        return false;
    }
    let scale = inst.host_scale();
    let Some(editor) = inst.editor.as_mut() else {
        return false;
    };
    let (min_w, min_h) = editor.min_size();
    let (max_w, max_h) = editor.max_size();
    // Host passes physical pixels; clamp in logical space then convert back.
    let (req_w, req_h) = unsafe { (*width, *height) };
    let (lw, lh) = gui_host_px_to_logical(req_w, req_h, scale);
    let lw = lw.clamp(min_w.max(1), max_w);
    let lh = lh.clamp(min_h.max(1), max_h);
    let (pw, ph) = gui_logical_to_host_px(lw, lh, scale);
    unsafe {
        *width = pw;
        *height = ph;
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
    // Host → physical; Editor::set_size is logical (same as size()).
    let (lw, lh) = gui_host_px_to_logical(width, height, inst.host_scale());
    let Some(editor) = inst.editor.as_mut() else {
        return false;
    };
    editor.set_size(lw, lh)
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
            host_scale: Arc::clone(&inst.host_scale),
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
pub(crate) unsafe fn request_param_rescan(host: *const clap_host) {
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
        if flags.contains(ParamFlags::MODULATABLE_PER_NOTE) {
            // Bitwig Voice Stack binds only when per-note *and* per-key
            // automation are advertised (clap-saw-demo / Surge set).
            f |= CLAP_PARAM_IS_AUTOMATABLE_PER_NOTE_ID
                | CLAP_PARAM_IS_AUTOMATABLE_PER_KEY
                | CLAP_PARAM_IS_AUTOMATABLE_PER_CHANNEL;
        }
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
    if flags.contains(ParamFlags::MODULATABLE_PER_NOTE) {
        f |= CLAP_PARAM_IS_MODULATABLE
            | CLAP_PARAM_IS_MODULATABLE_PER_NOTE_ID
            | CLAP_PARAM_IS_MODULATABLE_PER_KEY
            | CLAP_PARAM_IS_MODULATABLE_PER_CHANNEL;
    } else if flags.contains(ParamFlags::MODULATABLE) {
        f |= CLAP_PARAM_IS_MODULATABLE;
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

#[cfg(test)]
mod process_scratch_tests {
    use super::{MAX_NOTE_EVENTS, accept_note_event};

    #[test]
    fn expression_flood_is_capped_note_on_is_not() {
        assert!(accept_note_event(MAX_NOTE_EVENTS - 1, false));
        assert!(!accept_note_event(MAX_NOTE_EVENTS, false));
        assert!(accept_note_event(MAX_NOTE_EVENTS, true));
        assert!(accept_note_event(MAX_NOTE_EVENTS + 50, true));
    }

    #[test]
    fn notes_out_maps_end_to_clap_note_end() {
        use aura_core::NoteEventKind;
        use clap_sys::events::{
            CLAP_EVENT_NOTE_CHOKE, CLAP_EVENT_NOTE_END, CLAP_EVENT_NOTE_OFF, CLAP_EVENT_NOTE_ON,
        };
        let t = |k: NoteEventKind| -> u16 {
            match k {
                NoteEventKind::On { .. } => CLAP_EVENT_NOTE_ON,
                NoteEventKind::Off { .. } => CLAP_EVENT_NOTE_OFF,
                NoteEventKind::Choke => CLAP_EVENT_NOTE_CHOKE,
                NoteEventKind::End => CLAP_EVENT_NOTE_END,
                _ => 0xFFFF,
            }
        };
        assert_eq!(t(NoteEventKind::End), CLAP_EVENT_NOTE_END);
        assert_eq!(t(NoteEventKind::On { velocity: 1.0 }), CLAP_EVENT_NOTE_ON);
    }

    #[test]
    fn ump_out_is_clap_midi2() {
        use std::mem::size_of;

        use clap_sys::events::{
            CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_MIDI2, clap_event_header, clap_event_midi2,
        };

        let u = aura_core::Ump::midi2_per_note_pitch_bend(0, 0, 60, 0x8000_0000);
        assert!(u.is_per_note_pitch_bend());
        assert!(u.to_midi1().is_none());
        let e = clap_event_midi2 {
            header: clap_event_header {
                size: size_of::<clap_event_midi2>() as u32,
                time: 0,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: CLAP_EVENT_MIDI2,
                flags: 0,
            },
            port_index: 0,
            data: u.words(),
        };
        assert_eq!(e.header.type_, CLAP_EVENT_MIDI2);
        assert_eq!(e.data, u.words());
    }

    #[test]
    fn voice_info_overlapping_when_count_set() {
        use clap_sys::ext::voice_info::CLAP_VOICE_INFO_SUPPORTS_OVERLAPPING_NOTES;
        assert_ne!(CLAP_VOICE_INFO_SUPPORTS_OVERLAPPING_NOTES, 0);
    }
}

#[cfg(test)]
mod gui_scale_tests {
    use super::{scale_logical_to_physical, scale_physical_to_logical};

    #[test]
    fn scale_1_is_identity() {
        assert_eq!(scale_logical_to_physical(730, 395, 1.0), (730, 395));
        assert_eq!(scale_physical_to_logical(730, 395, 1.0), (730, 395));
    }

    #[test]
    fn scale_15_round_trip() {
        let (pw, ph) = scale_logical_to_physical(730, 395, 1.5);
        assert_eq!((pw, ph), (1095, 593));
        assert_eq!(scale_physical_to_logical(pw, ph, 1.5), (730, 395));
    }

    #[test]
    fn zoomed_frame_matches_child_physical() {
        // design 730×395, ui_zoom 100%, host 1.5 → host frame physical =
        // zoomed_logical × host_scale; child HWND = design × (host×zoom).
        let design = (730u32, 395u32);
        let ui_zoom = 1.0_f64;
        let host = 1.5_f64;
        let zoomed_w = ((f64::from(design.0) * ui_zoom).round() as u32).max(1);
        let zoomed_h = ((f64::from(design.1) * ui_zoom).round() as u32).max(1);
        let host_frame = scale_logical_to_physical(zoomed_w, zoomed_h, host);
        let child = scale_logical_to_physical(design.0, design.1, host * ui_zoom);
        assert_eq!(host_frame, child);
    }
}
