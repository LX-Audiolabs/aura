//! AURA core — minimal process / editor / plugin surface.
//!
//! Grown feature-by-feature. Not a full truce-core port.
//!
//! UI windowing lives in [`aura_baseview`]; host Editor adapter in
//! `aura-editor`. This crate only defines the host-facing [`Editor`]
//! trait and plugin process API that formats and the adapter will share.

pub mod buffer;
pub mod config;
pub mod editor;
pub mod events;
pub mod info;
pub mod plugin;
pub mod process;
pub mod state;
pub mod transport;

pub use buffer::AudioBuffer;
pub use config::{AudioConfig, ProcessMode};
pub use editor::{Editor, EditorBridge, IntoEditor, PluginContext, RawWindowHandle};
pub use events::{ParamEvent, ParamEventQueue};
pub use info::{MidiDialect, PluginCategory, PluginInfo};
pub use plugin::PluginLogic;
pub use process::{ProcessContext, ProcessStatus};
pub use state::{decode_state, encode_state};
pub use transport::Transport;
pub use aura_params::sample::{Float, Sample};
