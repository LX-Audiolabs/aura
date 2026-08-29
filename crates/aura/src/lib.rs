//! **AURA** — Audio Unified Rust Architecture.
//!
//! Umbrella crate for plugin authors. Prefer this over depending on
//! every `aura-*` piece individually:
//!
//! ```toml
//! aura = { path = ".../AURA/crates/aura", features = ["clap"] }
//! # optional UI:
//! aura-baseview = { path = "...", features = ["backend-femtovg"] }
//! aura-editor = { path = "...", features = ["backend-femtovg"] }
//! ```
//!
//! ```rust,ignore
//! use aura::prelude::*;
//! ```
//!
//! Features: `clap` / `vst3` / `lv2` for formats; `dsp` (default) for
//! `aura-dsp`. Params-only plugins: `default-features = false`.
//!
//! `#[derive(Params)]` requires this umbrella (or an equivalent
//! `aura_params` + path setup) — generated code uses `::aura::params::…`.

#![forbid(unsafe_code)]

pub use aura_core as core;
pub use aura_midi as midi;
pub use aura_params as params;

#[cfg(feature = "dsp")]
pub use aura_dsp as dsp;

#[cfg(feature = "clap")]
pub use aura_clap as clap;

#[cfg(feature = "vst3")]
pub use aura_vst3 as vst3;

#[cfg(feature = "lv2")]
pub use aura_lv2 as lv2;

// --- core surface ---
pub use aura_core::{
    AudioBuffer, AudioConfig, BusLayout, ChannelConfig, Editor, EditorBridge, FactoryPreset,
    FactoryPresetState, IntoEditor, MidiBuffer, MidiDialect, MidiEvent, MidiMessage, MidiStatus,
    NOTE_UNSPECIFIED, NoteBuffer, NoteEvent, NoteEventKind, NoteExpression, NoteNameEntry,
    NoteTarget, NoteVoice, NoteVoiceTable, ParamEvent, ParamEventQueue, PluginCategory,
    PluginContext, PluginInfo, PluginLogic, ProcessContext, ProcessMode, ProcessStatus,
    RawWindowHandle, Transport, Tuning, TuningEvent, TuningInfo, Ump, UmpBuffer, UmpEvent,
    append_midi_as_ump, append_notes_as_midi, append_ump_as_midi, apply_factory_preset,
    decode_state, encode_state, layout_at, load_preset_file,
};

// --- params surface ---
pub use aura_params::sample::{Float, Sample};
pub use aura_params::{
    AudioTap, BoolParam, EnumParam, FloatParam, FloatParamReadF32, FloatParamReadF64, IntParam,
    MeterSlot, MidiSource, ParamEnum, ParamFlags, ParamInfo, ParamRange, ParamUnit, ParamValueKind,
    Params, Smoother, SmoothingStyle, format_param_value, parse_param_value,
};

pub use aura_derive::{ParamEnum, Params};

#[cfg(feature = "clap")]
pub use aura_clap::export;

#[cfg(feature = "vst3")]
pub use aura_vst3::export_vst3;

#[cfg(feature = "lv2")]
pub use aura_lv2::export_lv2;

/// Common imports for plugin authors.
///
/// Niche helpers (`AudioTap`, `MeterSlot`, voice table) stay here so smokes
/// and instruments share one import line. Prefer `aura::encode_state` at the
/// crate root when you only need the codec.
pub mod prelude {
    pub use std::sync::Arc;

    pub use crate::{
        AudioBuffer, AudioConfig, AudioTap, BoolParam, BusLayout, ChannelConfig, Editor, EnumParam,
        FactoryPreset, FactoryPresetState, FloatParam, FloatParamReadF32, IntParam, IntoEditor,
        MeterSlot, MidiBuffer, MidiDialect, MidiMessage, NoteBuffer, NoteEvent, NoteEventKind,
        NoteExpression, NoteNameEntry, NoteTarget, NoteVoice, NoteVoiceTable, ParamEnum, ParamFlags,
        ParamInfo, ParamRange, ParamUnit, ParamValueKind, Params, PluginCategory, PluginContext,
        PluginInfo, PluginLogic, ProcessContext, ProcessMode, ProcessStatus, RawWindowHandle,
        SmoothingStyle, Transport, Ump, UmpBuffer, decode_state, encode_state, layout_at,
    };
    // FloatParamReadF64: same method names as F32 → E0034 if both in prelude.
    // Use `aura::FloatParamReadF64` explicitly.
    pub use crate::params::sample::Float;
}
