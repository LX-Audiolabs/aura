//! Minimal LV2 wrapper for AURA.
//!
//! **Spec:** [lv2plug.in](https://lv2plug.in/) — bindings [`lv2-sys`](https://crates.io/crates/lv2-sys).
//!
//! ```ignore
//! aura_lv2::export_lv2!(MyPlugin);
//! ```
//!
//! Bundle layout (written by `cargo aura install --lv2`):
//! ```text
//! name.lv2/
//!   manifest.ttl
//!   plugin.ttl
//!   <binary>          # smoke_gain.dll / libsmoke_gain.so / …
//! ```
//!
//! Port map follows the plugin's first `bus_layouts()` entry (LV2 is static):
//! mono → 0 in · 1 out · 2+ controls; stereo → 0/1 in · 2/3 out · 4+ controls.
//! When `PluginInfo::accepts_midi_in`, one `atom:Sequence` MIDI input port is
//! appended after controls; when `PluginInfo::emits_midi`, a matching output
//! port follows (audio/control indices unchanged).
//! State: shared [`aura_core::encode_state`] blob when the host maps URIDs.
//! GUI: LV2 UI extension via the same [`aura_core::editor::Editor`] trait as
//! CLAP/VST3, loaded through `lv2ui_descriptor` in the same binary.

#![allow(clippy::missing_safety_doc)]
#![allow(non_snake_case)]
// ponytail: LV2 FFI — raw pointers, similar audio channel names, format noise.
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
    clippy::if_not_else,
    clippy::items_after_statements,
    clippy::uninlined_format_args,
    clippy::needless_raw_string_hashes
)]

mod ttl;
mod ui;

pub use ttl::{BundleTtl, generate_ttl, generate_ttl_with_layout, generate_ttl_with_layout_and_ui};
pub use ui::ui_descriptor;

use std::ffi::{CStr, CString, c_char, c_void};
use std::ptr;
use std::sync::{Arc, OnceLock};

use aura_core::info::PluginInfo;
use aura_core::{
    AudioBuffer, AudioConfig, BusLayout, MidiBuffer, MidiMessage, MidiStatus, NoteBuffer,
    PluginLogic, ProcessContext, ProcessMode, append_notes_as_midi, decode_state, encode_state,
    host_callback, host_callback_with, layout_at,
};
use aura_params::Params;
use lv2_sys::{
    LV2_ATOM__Chunk, LV2_ATOM__Sequence, LV2_Atom, LV2_Atom_Event, LV2_Atom_Sequence,
    LV2_Descriptor, LV2_Feature, LV2_Handle, LV2_MIDI__MidiEvent, LV2_STATE__interface,
    LV2_State_Flags, LV2_State_Handle, LV2_State_Interface, LV2_State_Retrieve_Function,
    LV2_State_Status, LV2_State_Status_LV2_STATE_ERR_NO_FEATURE,
    LV2_State_Status_LV2_STATE_ERR_UNKNOWN, LV2_State_Status_LV2_STATE_SUCCESS,
    LV2_State_Store_Function, LV2_URID__map, LV2_URID_Map,
};

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

/// Hidden re-export for the macro.
#[doc(hidden)]
pub use lv2_sys as __lv2_sys;

/// Export `$logic` as this cdylib's LV2 entry (`lv2_descriptor` + `lv2ui_descriptor`).
#[macro_export]
macro_rules! export_lv2 {
    ($logic:ty) => {
        /// LV2 discovery entry — host calls with index 0 for the single plugin.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn lv2_descriptor(
            index: u32,
        ) -> *const $crate::__lv2_sys::LV2_Descriptor {
            $crate::descriptor::<$logic>(index)
        }

        /// LV2 UI discovery entry — host calls with index 0 for the single UI.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn lv2ui_descriptor(
            index: u32,
        ) -> *const $crate::__lv2_sys::LV2UI_Descriptor {
            $crate::ui_descriptor::<$logic>(index)
        }
    };
}

// ---------------------------------------------------------------------------
// URI / metadata helpers
// ---------------------------------------------------------------------------

/// Plugin URI: `PluginInfo::lv2_uri` or derived `https://lx-audiolabs.com/lv2/<bundle_id>`.
pub fn plugin_uri(info: &PluginInfo) -> String {
    if !info.lv2_uri.is_empty() {
        info.lv2_uri.to_string()
    } else {
        format!("https://lx-audiolabs.com/lv2/{}", info.bundle_id)
    }
}

fn uri_cstring<L: PluginLogic>() -> &'static CStr {
    static CELL: OnceLock<CString> = OnceLock::new();
    CELL.get_or_init(|| {
        let s = plugin_uri(&L::info());
        CString::new(s).expect("plugin URI has interior NUL")
    })
    .as_c_str()
}

fn param_list<L: PluginLogic>() -> Vec<aura_params::ParamInfo> {
    let static_list = L::Params::param_infos_static();
    if static_list.is_empty() {
        L::Params::default().param_infos()
    } else {
        static_list
    }
}

// ---------------------------------------------------------------------------
// Instance
// ---------------------------------------------------------------------------

/// Static layout for LV2 (first declared `bus_layouts()` entry).
fn static_layout<L: PluginLogic>() -> BusLayout {
    layout_at(&L::bus_layouts(), 0)
}

fn audio_port_count(layout: BusLayout) -> usize {
    layout.total_input_channels() as usize + layout.main_output_channels() as usize
}

struct Instance<L: PluginLogic> {
    params: Arc<L::Params>,
    state: Option<L::DspState>,
    sample_rate: f64,
    /// Audio + control port data locations (host-owned).
    ports: Vec<*mut c_void>,
    state_key: u32,
    chunk_type: u32,
    layout: BusLayout,
    /// First control-port index (= audio port count).
    ctrl0: u32,
    /// Atom-sequence MIDI input port index when the plugin accepts MIDI.
    midi_in_port: Option<u32>,
    /// Atom-sequence MIDI output port index when the plugin emits MIDI.
    midi_out_port: Option<u32>,
    /// URID for `midi:MidiEvent` (0 when unmapped / no MIDI port).
    midi_event_type: u32,
    /// URID for `atom:Sequence` (0 when unmapped / no MIDI port).
    sequence_type: u32,
    scratch: ProcessScratch,
}

/// Audio-thread working set. Reserved in `activate`.
struct ProcessScratch {
    midi: MidiBuffer,
    midi_out: MidiBuffer,
    notes_out: NoteBuffer,
    silence: Vec<f32>,
}

const MAX_AUDIO_CH: usize = 8;
const MAX_MIDI_EVENTS: usize = 4096;
/// LV2 has no host max-block; reserve a generous default and grow once if needed.
const LV2_SCRATCH_FRAMES: usize = 8192;

impl ProcessScratch {
    fn new() -> Self {
        Self {
            midi: MidiBuffer::new(),
            midi_out: MidiBuffer::new(),
            notes_out: NoteBuffer::new(),
            silence: Vec::new(),
        }
    }

    fn prepare(&mut self, max_frames: usize) {
        self.midi.reserve(MAX_MIDI_EVENTS + 128);
        self.midi_out.reserve(256);
        self.notes_out.reserve(256);
        if self.silence.len() < max_frames {
            self.silence.resize(max_frames, 0.0);
        }
    }
}

impl<L: PluginLogic> Instance<L> {
    fn from_handle<'a>(handle: LV2_Handle) -> Option<&'a mut Self> {
        if handle.is_null() {
            return None;
        }
        Some(unsafe { &mut *(handle as *mut Self) })
    }

    fn n_ports() -> usize {
        audio_port_count(static_layout::<L>())
            + param_list::<L>().len()
            + usize::from(L::info().accepts_midi_in)
            + usize::from(L::info().emits_midi)
    }
}

// ---------------------------------------------------------------------------
// Descriptor callbacks
// ---------------------------------------------------------------------------

/// Public for the export macro / tests.
#[must_use]
pub fn descriptor<L: PluginLogic>(index: u32) -> *const LV2_Descriptor {
    if index != 0 {
        return ptr::null();
    }
    // OnceLock needs Sync; LV2_Descriptor has raw pointers. Box + leak is fine
    // for a process-lifetime descriptor (same pattern many LV2 plugins use).
    static CELL: OnceLock<usize> = OnceLock::new();
    let addr = *CELL.get_or_init(|| {
        let desc = Box::new(LV2_Descriptor {
            URI: uri_cstring::<L>().as_ptr(),
            instantiate: Some(instantiate::<L>),
            connect_port: Some(connect_port::<L>),
            activate: Some(activate::<L>),
            run: Some(run::<L>),
            deactivate: Some(deactivate::<L>),
            cleanup: Some(cleanup::<L>),
            extension_data: Some(extension_data::<L>),
        });
        Box::into_raw(desc) as usize
    });
    addr as *const LV2_Descriptor
}

unsafe extern "C" fn instantiate<L: PluginLogic>(
    _descriptor: *const LV2_Descriptor,
    sample_rate: f64,
    _bundle_path: *const c_char,
    features: *const *const LV2_Feature,
) -> LV2_Handle {
    let params = Arc::new(L::Params::default());
    params.set_sample_rate(sample_rate);
    let layout = static_layout::<L>();
    let n = Instance::<L>::n_ports();
    let ctrl0 = audio_port_count(layout) as u32;
    let mut map_ptr: Option<*const LV2_URID_Map> = None;
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
                if u.to_bytes() == &LV2_URID__map[..LV2_URID__map.len() - 1] {
                    map_ptr = Some(unsafe { (*f).data as *const LV2_URID_Map });
                }
            }
            i += 1;
        }
    }

    let (state_key, chunk_type) = map_ptr
        .and_then(|m| unsafe { map_state_urids(m, &L::info()) })
        .unwrap_or((0, 0));

    let info = L::info();
    // MIDI atom ports sit after controls — audio/control indices unchanged.
    let mut midi_in_port = None;
    let mut midi_out_port = None;
    let mut next_midi = audio_port_count(layout) + param_list::<L>().len();
    if info.accepts_midi_in {
        midi_in_port = Some(next_midi as u32);
        next_midi += 1;
    }
    if info.emits_midi {
        midi_out_port = Some(next_midi as u32);
    }
    let (midi_event_type, sequence_type) = match (midi_in_port.or(midi_out_port), map_ptr) {
        (Some(_), Some(m)) => unsafe {
            (
                map_urid(m, LV2_MIDI__MidiEvent),
                map_urid(m, LV2_ATOM__Sequence),
            )
        },
        _ => (0, 0),
    };

    let inst = Box::new(Instance::<L> {
        params,
        state: None,
        sample_rate,
        ports: vec![ptr::null_mut(); n],
        state_key,
        chunk_type,
        layout,
        ctrl0,
        midi_in_port,
        midi_out_port,
        midi_event_type,
        sequence_type,
        scratch: ProcessScratch::new(),
    });
    Box::into_raw(inst) as LV2_Handle
}

/// Map one URI to a URID; 0 on failure.
unsafe fn map_urid(map: *const LV2_URID_Map, uri: &'static [u8]) -> u32 {
    let map = unsafe { &*map };
    let Some(map_fn) = map.map else {
        return 0;
    };
    unsafe { map_fn(map.handle, uri.as_ptr().cast::<c_char>()) }
}

unsafe fn map_state_urids(map: *const LV2_URID_Map, info: &PluginInfo) -> Option<(u32, u32)> {
    let map = unsafe { &*map };
    let map_fn = map.map?;
    let key_uri = CString::new(format!("{}#state", plugin_uri(info))).ok()?;
    let key = unsafe { map_fn(map.handle, key_uri.as_ptr()) };
    let chunk = unsafe { map_fn(map.handle, LV2_ATOM__Chunk.as_ptr().cast::<c_char>()) };
    if key == 0 || chunk == 0 {
        return None;
    }
    Some((key, chunk))
}

unsafe extern "C" fn connect_port<L: PluginLogic>(
    instance: LV2_Handle,
    port: u32,
    data_location: *mut c_void,
) {
    let Some(inst) = Instance::<L>::from_handle(instance) else {
        return;
    };
    if let Some(slot) = inst.ports.get_mut(port as usize) {
        *slot = data_location;
    }
}

unsafe extern "C" fn activate<L: PluginLogic>(instance: LV2_Handle) {
    let Some(inst) = Instance::<L>::from_handle(instance) else {
        return;
    };
    // Block size unknown until first run — use a generous default for reset.
    let config = AudioConfig::new(inst.sample_rate, 8192)
        .with_process_mode(ProcessMode::Realtime)
        .with_channels(
            inst.layout.main_input_channels(),
            inst.layout.main_output_channels(),
        )
        .with_sidechain_channels(inst.layout.sidechain_input_channels());
    let mut dsp = L::init(&inst.params, inst.sample_rate);
    L::reset(&mut dsp, &inst.params, &config);
    inst.scratch.prepare(LV2_SCRATCH_FRAMES);
    inst.state = Some(dsp);
}

unsafe extern "C" fn deactivate<L: PluginLogic>(instance: LV2_Handle) {
    if let Some(inst) = Instance::<L>::from_handle(instance) {
        inst.state = None;
    }
}

unsafe extern "C" fn cleanup<L: PluginLogic>(instance: LV2_Handle) {
    if !instance.is_null() {
        drop(unsafe { Box::from_raw(instance as *mut Instance<L>) });
    }
}

unsafe extern "C" fn run<L: PluginLogic>(instance: LV2_Handle, sample_count: u32) {
    host_callback("LV2", "run", || {
        let Some(inst) = Instance::<L>::from_handle(instance) else {
            return;
        };
        let n = sample_count as usize;
        if n == 0 {
            return;
        }
        if n > inst.scratch.silence.len() {
            // Host used a larger block than activate reserved — grow once.
            inst.scratch.prepare(n);
        }

        // Control ports → params (plain floats).
        let infos = param_list::<L>();
        let ctrl0 = inst.ctrl0;
        for (i, meta) in infos.iter().enumerate() {
            let port = ctrl0 + i as u32;
            let ptr = inst
                .ports
                .get(port as usize)
                .copied()
                .unwrap_or(ptr::null_mut());
            if !ptr.is_null() {
                let v = unsafe { *(ptr as *const f32) };
                inst.params.set_plain(meta.id, f64::from(v));
            }
        }

        inst.scratch.midi.clear();
        inst.scratch.midi_out.clear();
        inst.scratch.notes_out.clear();
        read_midi(inst, n);
        if inst.state.is_none() {
            return;
        }

        let in_ch = inst.layout.main_input_channels() as usize;
        let sidechain_ch = inst.layout.sidechain_input_channels() as usize;
        let total_in_ch = in_ch + sidechain_ch;
        let out_ch = inst.layout.main_output_channels() as usize;
        if !matches!(out_ch, 1 | 2) || total_in_ch > MAX_AUDIO_CH {
            return;
        }

        // Distinct fields; raw slice so we can also mut-borrow `state`.
        let silence = unsafe { std::slice::from_raw_parts(inst.scratch.silence.as_ptr(), n) };
        let mut in_store = [&[] as &[f32]; MAX_AUDIO_CH];
        for i in 0..total_in_ch {
            in_store[i] = port_audio(inst.ports.get(i).copied(), n).unwrap_or(silence);
        }
        let in_refs = &in_store[..total_in_ch];

        let mut out_ptrs = [ptr::null_mut::<f32>(); MAX_AUDIO_CH];
        for (i, dest) in out_ptrs.iter_mut().take(out_ch).enumerate() {
            let port_i = total_in_ch + i;
            *dest = inst
                .ports
                .get(port_i)
                .copied()
                .unwrap_or(ptr::null_mut())
                .cast::<f32>();
        }
        if out_ptrs[..out_ch].iter().any(|p| p.is_null()) {
            return;
        }

        let dsp = inst.state.as_mut().expect("checked is_none above");
        let mut ctx = ProcessContext::new(inst.sample_rate, n)
            .with_process_mode(ProcessMode::Realtime)
            .with_midi(std::mem::take(&mut inst.scratch.midi))
            .with_midi_out(std::mem::take(&mut inst.scratch.midi_out))
            .with_notes_out(std::mem::take(&mut inst.scratch.notes_out));
        let _ = unsafe {
            run_process_chunk::<L>(
                dsp,
                &inst.params,
                in_refs,
                out_ptrs,
                out_ch,
                n,
                in_ch,
                sidechain_ch,
                &mut ctx,
            )
        };
        inst.scratch.midi = std::mem::take(&mut ctx.midi);
        append_notes_as_midi(&mut ctx.midi_out, &ctx.notes_out);
        write_midi(inst, n, &ctx.midi_out);
        inst.scratch.midi_out = std::mem::take(&mut ctx.midi_out);
        inst.scratch.midi_out.clear();
        inst.scratch.notes_out = std::mem::take(&mut ctx.notes_out);
        inst.scratch.notes_out.clear();
    });
}

/// Write one block into host port pointers (mono or stereo).
#[allow(clippy::too_many_arguments)]
unsafe fn run_process_chunk<L: PluginLogic>(
    state: &mut L::DspState,
    params: &L::Params,
    in_refs: &[&[f32]],
    out_ptrs: [*mut f32; MAX_AUDIO_CH],
    out_ch: usize,
    frames: usize,
    main_in_ch: usize,
    sidechain_in_ch: usize,
    ctx: &mut ProcessContext,
) -> aura_core::ProcessStatus {
    match out_ch {
        1 => {
            let mut s0 = unsafe { std::slice::from_raw_parts_mut(out_ptrs[0], frames) };
            let mut outs = [&mut s0 as &mut [f32]];
            let mut buffer = unsafe {
                AudioBuffer::from_slices_with_sidechain_unchecked(
                    in_refs,
                    &mut outs,
                    frames,
                    main_in_ch,
                    sidechain_in_ch,
                )
            };
            L::process(state, params, &mut buffer, ctx)
        }
        2 => {
            let mut s0 = unsafe { std::slice::from_raw_parts_mut(out_ptrs[0], frames) };
            let mut s1 = unsafe { std::slice::from_raw_parts_mut(out_ptrs[1], frames) };
            let mut outs = [&mut s0 as &mut [f32], &mut s1];
            let mut buffer = unsafe {
                AudioBuffer::from_slices_with_sidechain_unchecked(
                    in_refs,
                    &mut outs,
                    frames,
                    main_in_ch,
                    sidechain_in_ch,
                )
            };
            L::process(state, params, &mut buffer, ctx)
        }
        _ => aura_core::ProcessStatus::Error,
    }
}

fn port_audio<'a>(ptr: Option<*mut c_void>, n: usize) -> Option<&'a [f32]> {
    let p = ptr.filter(|p| !p.is_null())?;
    Some(unsafe { std::slice::from_raw_parts(p as *const f32, n) })
}

/// Encode [`MidiBuffer`] events into the MIDI atom-sequence output port.
///
/// Writes raw MIDI bytes (status + up to two data bytes) as `midi:MidiEvent`
/// atoms into the host-provided output sequence buffer. Silently drops events
/// when the host buffer is too small or unmapped.
fn write_midi<L: PluginLogic>(inst: &Instance<L>, n: usize, midi: &MidiBuffer) {
    let Some(port) = inst.midi_out_port else {
        return;
    };
    if inst.midi_event_type == 0 || inst.sequence_type == 0 {
        return;
    }
    let ptr = inst
        .ports
        .get(port as usize)
        .copied()
        .unwrap_or(ptr::null_mut());
    if ptr.is_null() {
        return;
    }
    if midi.is_empty() {
        unsafe {
            let seq = &mut *(ptr as *mut LV2_Atom_Sequence);
            seq.atom.type_ = inst.sequence_type;
            seq.atom.size = (size_of::<LV2_Atom_Sequence>() - size_of::<LV2_Atom>()) as u32;
            seq.body.unit = 0;
            seq.body.pad = 0;
        }
        return;
    }

    unsafe {
        let seq = &mut *(ptr as *mut LV2_Atom_Sequence);
        let base = ptr as *mut u8;
        // Use the host-provided size as capacity when non-zero; otherwise fall
        // back to a conservative default (hosts typically reserve ≥ 4 KiB).
        let capacity = if seq.atom.size > 0 {
            size_of::<LV2_Atom>() + seq.atom.size as usize
        } else {
            4096
        };

        seq.atom.type_ = inst.sequence_type;
        seq.body.unit = 0;
        seq.body.pad = 0;

        let header_size = size_of::<LV2_Atom_Sequence>();
        let mut off = header_size;
        for ev in midi.iter() {
            let (bytes, len) = message_bytes(ev.message);
            let event_size = size_of::<LV2_Atom_Event>() + len;
            let padded = (event_size + 7) & !7;
            if off + padded > capacity {
                break; // host buffer full
            }

            let atom_ev = base.add(off) as *mut LV2_Atom_Event;
            (*atom_ev).time.frames = ev.sample_offset.min(n as u32 - 1) as i64;
            (*atom_ev).body.type_ = inst.midi_event_type;
            (*atom_ev).body.size = len as u32;
            let data_ptr = base.add(off + size_of::<LV2_Atom_Event>());
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), data_ptr, len);

            off += padded;
        }
        seq.atom.size = (off - size_of::<LV2_Atom>()) as u32;
    }
}

/// Number of raw bytes this message occupies on the wire.
fn message_len(msg: MidiMessage) -> usize {
    match msg.status {
        MidiStatus::ProgramChange | MidiStatus::ChannelPressure => 2,
        MidiStatus::System => 1,
        _ => 3,
    }
}

/// Raw MIDI bytes for a message. Only the first [`message_len`] bytes are valid.
fn message_bytes(msg: MidiMessage) -> ([u8; 3], usize) {
    let len = message_len(msg);
    ([msg.status_byte(), msg.data1, msg.data2], len)
}

/// Parse the MIDI atom-sequence input port into `inst.scratch.midi`.
///
/// No-op when the plugin has no MIDI port, the host left it unconnected,
/// URID mapping failed, or the buffer is not an `atom:Sequence`.
fn read_midi<L: PluginLogic>(inst: &mut Instance<L>, n: usize) {
    let Some(port) = inst.midi_in_port else {
        return;
    };
    if inst.midi_event_type == 0 {
        return;
    }
    let ptr = inst
        .ports
        .get(port as usize)
        .copied()
        .unwrap_or(ptr::null_mut());
    if ptr.is_null() {
        return;
    }
    let midi_event_type = inst.midi_event_type;
    let sequence_type = inst.sequence_type;
    let midi = &mut inst.scratch.midi;
    unsafe {
        let seq = &*(ptr as *const LV2_Atom_Sequence);
        if sequence_type != 0 && seq.atom.type_ != sequence_type {
            return;
        }
        let base = ptr as *const u8;
        // atom.size covers everything after the LV2_Atom header (seq body + events).
        let total = size_of::<LV2_Atom>() + seq.atom.size as usize;
        let mut off = size_of::<LV2_Atom_Sequence>();
        while off + size_of::<LV2_Atom_Event>() <= total {
            let ev = &*(base.add(off) as *const LV2_Atom_Event);
            let body_size = ev.body.size as usize;
            let data_off = off + size_of::<LV2_Atom_Event>();
            if data_off + body_size > total {
                break; // corrupt buffer — stop
            }
            if ev.body.type_ == midi_event_type && body_size > 0 {
                if midi.len() >= MAX_MIDI_EVENTS {
                    break;
                }
                // LV2 MIDI events are 1-3 raw bytes, status first (no running status).
                let data = std::slice::from_raw_parts(base.add(data_off), body_size.min(3));
                let offset = ev.time.frames.clamp(0, n as i64 - 1) as u32;
                midi.push(
                    offset,
                    MidiMessage::raw(
                        data[0],
                        data.get(1).copied().unwrap_or(0),
                        data.get(2).copied().unwrap_or(0),
                    ),
                );
            }
            // Events are 64-bit aligned: header + body padded to 8 bytes.
            off = data_off + ((body_size + 7) & !7);
        }
    }
}

// ---------------------------------------------------------------------------
// State extension
// ---------------------------------------------------------------------------

unsafe extern "C" fn extension_data<L: PluginLogic>(uri: *const c_char) -> *const c_void {
    if uri.is_null() {
        return ptr::null();
    }
    let u = unsafe { CStr::from_ptr(uri) };
    if u.to_bytes() == &LV2_STATE__interface[..LV2_STATE__interface.len() - 1] {
        return state_iface::<L>() as *const _ as *const c_void;
    }
    ptr::null()
}

fn state_iface<L: PluginLogic>() -> &'static LV2_State_Interface {
    static CELL: OnceLock<LV2_State_Interface> = OnceLock::new();
    CELL.get_or_init(|| LV2_State_Interface {
        save: Some(state_save::<L>),
        restore: Some(state_restore::<L>),
    })
}

unsafe extern "C" fn state_save<L: PluginLogic>(
    instance: LV2_Handle,
    store: LV2_State_Store_Function,
    handle: LV2_State_Handle,
    _flags: u32,
    _features: *const *const LV2_Feature,
) -> LV2_State_Status {
    host_callback_with(
        "LV2",
        "state_save",
        LV2_State_Status_LV2_STATE_ERR_UNKNOWN,
        || {
            let Some(inst) = Instance::<L>::from_handle(instance) else {
                return LV2_State_Status_LV2_STATE_ERR_UNKNOWN;
            };
            let Some(store) = store else {
                return LV2_State_Status_LV2_STATE_ERR_NO_FEATURE;
            };
            if inst.state_key == 0 || inst.chunk_type == 0 {
                return LV2_State_Status_LV2_STATE_ERR_NO_FEATURE;
            }
            let blob = encode_state(&*inst.params);
            if blob.is_empty() {
                return LV2_State_Status_LV2_STATE_SUCCESS;
            }
            let flags = (LV2_State_Flags::LV2_STATE_IS_POD.0
                | LV2_State_Flags::LV2_STATE_IS_PORTABLE.0) as u32;
            unsafe {
                store(
                    handle,
                    inst.state_key,
                    blob.as_ptr().cast(),
                    blob.len(),
                    inst.chunk_type,
                    flags,
                )
            }
        },
    )
}

unsafe extern "C" fn state_restore<L: PluginLogic>(
    instance: LV2_Handle,
    retrieve: LV2_State_Retrieve_Function,
    handle: LV2_State_Handle,
    _flags: u32,
    _features: *const *const LV2_Feature,
) -> LV2_State_Status {
    host_callback_with(
        "LV2",
        "state_restore",
        LV2_State_Status_LV2_STATE_ERR_UNKNOWN,
        || {
            let Some(inst) = Instance::<L>::from_handle(instance) else {
                return LV2_State_Status_LV2_STATE_ERR_UNKNOWN;
            };
            let Some(retrieve) = retrieve else {
                return LV2_State_Status_LV2_STATE_ERR_NO_FEATURE;
            };
            if inst.state_key == 0 {
                return LV2_State_Status_LV2_STATE_ERR_NO_FEATURE;
            }
            let mut size = 0usize;
            let mut type_ = 0u32;
            let mut flags = 0u32;
            let ptr =
                unsafe { retrieve(handle, inst.state_key, &mut size, &mut type_, &mut flags) };
            if ptr.is_null() || size == 0 {
                return LV2_State_Status_LV2_STATE_SUCCESS; // empty = defaults
            }
            let slice = unsafe { std::slice::from_raw_parts(ptr as *const u8, size) };
            if decode_state(&*inst.params, slice) {
                LV2_State_Status_LV2_STATE_SUCCESS
            } else {
                LV2_State_Status_LV2_STATE_ERR_UNKNOWN
            }
        },
    )
}

// ---------------------------------------------------------------------------
// Bundle helpers (install)
// ---------------------------------------------------------------------------

/// Generate TTL for `$logic` and the given shared-library stem (crate name with `-` → `_`).
pub fn bundle_ttl_for<L: PluginLogic>(binary_stem: &str) -> BundleTtl {
    let info = L::info();
    let uri = plugin_uri(&info);
    let params = param_list::<L>();
    let layout = static_layout::<L>();
    let has_ui = L::editor(Arc::new(L::Params::default())).is_some();
    generate_ttl_with_layout_and_ui(&info, &uri, binary_stem, &params, layout, has_ui)
}

/// Non-generic install helper: TTL from free functions + binary stem.
///
/// Defaults to stereo main I/O when the caller has no layout.
pub fn bundle_ttl_from_parts(
    info: &PluginInfo,
    params: &[aura_params::ParamInfo],
    binary_stem: &str,
) -> BundleTtl {
    let uri = plugin_uri(info);
    generate_ttl(info, &uri, binary_stem, params)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aura_core::PluginInfo;

    #[test]
    fn plugin_uri_fallback() {
        let mut info = PluginInfo::new("X", "V", "0.1", "my-plug");
        assert_eq!(plugin_uri(&info), "https://lx-audiolabs.com/lv2/my-plug");
        info.lv2_uri = "https://example.com/lv2/x";
        assert_eq!(plugin_uri(&info), "https://example.com/lv2/x");
    }
}
