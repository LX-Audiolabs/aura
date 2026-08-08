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
//! appended last (audio/control indices unchanged).
//! State: shared [`aura_core::encode_state`] blob when the host maps URIDs.
//! No GUI in v1 (LV2 UI is a separate story).

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

pub use ttl::{BundleTtl, generate_ttl, generate_ttl_with_layout};

use std::ffi::{CStr, CString, c_char, c_void};
use std::ptr;
use std::sync::{Arc, OnceLock};

use aura_core::{
    AudioBuffer, AudioConfig, BusLayout, MidiBuffer, MidiMessage, PluginLogic, ProcessContext,
    ProcessMode, decode_state, encode_state, host_callback, host_callback_with, layout_at,
};
use aura_core::info::PluginInfo;
use aura_params::Params;
use lv2_sys::{
    LV2_ATOM__Chunk, LV2_ATOM__Sequence, LV2_Atom, LV2_Atom_Event, LV2_Atom_Sequence,
    LV2_Descriptor, LV2_Feature, LV2_Handle, LV2_MIDI__MidiEvent, LV2_STATE__interface,
    LV2_State_Flags, LV2_State_Handle, LV2_State_Interface, LV2_State_Retrieve_Function,
    LV2_State_Status, LV2_State_Status_LV2_STATE_ERR_NO_FEATURE,
    LV2_State_Status_LV2_STATE_ERR_UNKNOWN, LV2_State_Status_LV2_STATE_SUCCESS,
    LV2_State_Store_Function, LV2_URID_Map, LV2_URID__map,
};

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

/// Hidden re-export for the macro.
#[doc(hidden)]
pub use lv2_sys as __lv2_sys;

/// Export `$logic` as this cdylib's LV2 entry (`lv2_descriptor`).
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
    layout.main_input_channels() as usize + layout.main_output_channels() as usize
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
    /// Atom-sequence MIDI input port index (last port) when the plugin accepts MIDI.
    midi_port: Option<u32>,
    /// URID for `midi:MidiEvent` (0 when unmapped / no MIDI port).
    midi_event_type: u32,
    /// URID for `atom:Sequence` (0 when unmapped / no MIDI port).
    sequence_type: u32,
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

    // MIDI atom port sits last (after controls) — audio/control indices unchanged.
    let midi_port = L::info()
        .accepts_midi_in
        .then_some(n.saturating_sub(1) as u32);
    let (midi_event_type, sequence_type) = match (midi_port, map_ptr) {
        (Some(_), Some(m)) => unsafe {
            (map_urid(m, LV2_MIDI__MidiEvent), map_urid(m, LV2_ATOM__Sequence))
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
        midi_port,
        midi_event_type,
        sequence_type,
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
        );
    let mut dsp = L::init(&inst.params, inst.sample_rate);
    L::reset(&mut dsp, &inst.params, &config);
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

        // Control ports → params (plain floats).
        let infos = param_list::<L>();
        let ctrl0 = inst.ctrl0;
        for (i, meta) in infos.iter().enumerate() {
            let port = ctrl0 + i as u32;
            let ptr = inst.ports.get(port as usize).copied().unwrap_or(ptr::null_mut());
            if !ptr.is_null() {
                let v = unsafe { *(ptr as *const f32) };
                inst.params.set_plain(meta.id, f64::from(v));
            }
        }

        // MIDI atom input → MidiBuffer (before the mutable dsp borrow).
        let midi = read_midi(inst, n);

        let Some(dsp) = inst.state.as_mut() else {
            return;
        };

        let in_ch = inst.layout.main_input_channels() as usize;
        let out_ch = inst.layout.main_output_channels() as usize;
        if out_ch == 0 {
            return;
        }

        // Copy host inputs (or silence) into owned buffers.
        let mut owned_in: Vec<Vec<f32>> = Vec::with_capacity(out_ch);
        for c in 0..out_ch {
            if c < in_ch {
                if let Some(s) = port_audio(inst.ports.get(c).copied(), n) {
                    owned_in.push(s.to_vec());
                } else {
                    owned_in.push(vec![0.0; n]);
                }
            } else {
                owned_in.push(vec![0.0; n]);
            }
        }

        let mut owned_out = owned_in.clone();
        {
            let in_refs: Vec<&[f32]> = owned_in.iter().map(Vec::as_slice).collect();
            let mut out_refs: Vec<&mut [f32]> =
                owned_out.iter_mut().map(Vec::as_mut_slice).collect();
            let mut buffer = AudioBuffer::from_slices_checked(&in_refs, &mut out_refs, n);
            let mut ctx = ProcessContext::new(inst.sample_rate, n)
                .with_process_mode(ProcessMode::Realtime)
                .with_midi(midi);
            let _ = L::process(dsp, &inst.params, &mut buffer, &mut ctx);
        }

        for (c, out_ch_buf) in owned_out.iter().enumerate() {
            let port_i = in_ch + c;
            if let Some(s) = port_audio_mut(inst.ports.get(port_i).copied(), n) {
                s.copy_from_slice(out_ch_buf);
            }
        }
    });
}

fn port_audio<'a>(ptr: Option<*mut c_void>, n: usize) -> Option<&'a [f32]> {
    let p = ptr.filter(|p| !p.is_null())?;
    Some(unsafe { std::slice::from_raw_parts(p as *const f32, n) })
}

fn port_audio_mut<'a>(ptr: Option<*mut c_void>, n: usize) -> Option<&'a mut [f32]> {
    let p = ptr.filter(|p| !p.is_null())?;
    Some(unsafe { std::slice::from_raw_parts_mut(p as *mut f32, n) })
}

/// Parse the MIDI atom-sequence input port into a [`MidiBuffer`].
///
/// Empty when the plugin has no MIDI port, the host left it unconnected,
/// URID mapping failed, or the buffer is not an `atom:Sequence`.
fn read_midi<L: PluginLogic>(inst: &Instance<L>, n: usize) -> MidiBuffer {
    let mut midi = MidiBuffer::new();
    let Some(port) = inst.midi_port else {
        return midi;
    };
    if inst.midi_event_type == 0 {
        return midi;
    }
    let ptr = inst.ports.get(port as usize).copied().unwrap_or(ptr::null_mut());
    if ptr.is_null() {
        return midi;
    }
    unsafe {
        let seq = &*(ptr as *const LV2_Atom_Sequence);
        if inst.sequence_type != 0 && seq.atom.type_ != inst.sequence_type {
            return midi;
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
            if ev.body.type_ == inst.midi_event_type && body_size > 0 {
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
    midi
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
            let flags =
                (LV2_State_Flags::LV2_STATE_IS_POD.0 | LV2_State_Flags::LV2_STATE_IS_PORTABLE.0)
                    as u32;
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
    generate_ttl_with_layout(&info, &uri, binary_stem, &params, layout)
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
