//! **AURA** — Audio Unified Rust Architecture.
//!
//! Umbrella crate for plugin authors. Prefer this over depending on
//! every `aura-*` piece individually:
//!
//! ```toml
//! aura = { path = ".../AURA/crates/aura" }
//! aura-baseview = { path = ".../AURA/crates/aura-baseview", features = ["backend-femtovg"] }
//! aura-editor = { path = ".../AURA/crates/aura-editor", features = ["backend-femtovg"] }
//! ```
//!
//! ```rust,ignore
//! use aura::prelude::*;
//! ```
//!
//! Enable formats with features, same idea as the `truce` umbrella:
//!
//! ```toml
//! aura = { path = "...", features = ["clap", "vst3"] }
//! ```

#![forbid(unsafe_code)]

pub use aura_core as core;
pub use aura_params as params;

#[cfg(feature = "clap")]
pub use aura_clap as clap;

#[cfg(feature = "vst3")]
pub use aura_vst3 as vst3;

#[cfg(feature = "lv2")]
pub use aura_lv2 as lv2;

// --- core surface ---
pub use aura_core::{
    AudioBuffer, AudioConfig, Editor, EditorBridge, IntoEditor, MidiDialect, ParamEvent,
    ParamEventQueue, PluginCategory, PluginContext, PluginInfo, PluginLogic, ProcessContext,
    ProcessMode, ProcessStatus, RawWindowHandle, Transport,
};

// --- params surface ---
pub use aura_params::{
    BoolParam, EnumParam, FloatParam, IntParam, MeterSlot, MidiSource, ParamEnum, ParamFlags,
    ParamInfo, ParamRange, ParamUnit, ParamValueKind, Params, Smoother, SmoothingStyle,
    format_param_value,
};
pub use aura_params::sample::{Float, Sample};

// --- derive macros (macro namespace; `Params` / `ParamEnum` coexist
// with the same-named traits, serde-style) ---
pub use aura_derive::{ParamEnum, Params};

/// Re-export CLAP export macro when `clap` feature is on.
#[cfg(feature = "clap")]
pub use aura_clap::export;

/// Re-export VST3 export macro when `vst3` feature is on.
#[cfg(feature = "vst3")]
pub use aura_vst3::export_vst3;

/// Re-export LV2 export macro when `lv2` feature is on.
#[cfg(feature = "lv2")]
pub use aura_lv2::export_lv2;

/// Common imports for plugin crates (grows with the framework).
pub mod prelude {
    pub use std::sync::Arc;

    pub use crate::{
        AudioBuffer, AudioConfig, BoolParam, Editor, EnumParam, FloatParam, IntParam, IntoEditor,
        MeterSlot, ParamEnum, ParamInfo, ParamRange, ParamUnit, ParamValueKind, Params,
        PluginCategory, PluginContext, PluginInfo, PluginLogic, ProcessContext, ProcessMode,
        ProcessStatus, RawWindowHandle, SmoothingStyle,
    };
    pub use crate::params::sample::Float;
}
