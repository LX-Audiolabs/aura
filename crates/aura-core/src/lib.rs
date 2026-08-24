//! AURA core — minimal process / editor / plugin surface.
//!
//! UI windowing lives in [`aura_baseview`]; host Editor adapter in
//! `aura-editor`. This crate only defines the host-facing [`Editor`]
//! trait and plugin process API that formats and the adapter will share.

pub mod buffer;
pub mod bus;
pub mod chunked_process;
pub mod config;
pub mod editor;
pub mod events;
pub mod host_fence;
pub mod info;
pub mod note_events;
pub mod note_voices;
pub mod plugin;
pub mod preset;
pub mod process;
pub mod state;
pub mod transport;
pub mod tuning;

pub use aura_params::sample::{Float, Sample};
pub use buffer::AudioBuffer;
pub use bus::{BusLayout, ChannelConfig, layout_at};
pub use chunked_process::{
    TimedParamEvent, apply_at_time, apply_event, apply_non_chunked, is_split_event, split_points,
    split_points_into,
};
pub use config::{AudioConfig, ProcessMode};
pub use editor::{Editor, EditorBridge, IntoEditor, PluginContext, RawWindowHandle};
pub use events::{ParamEvent, ParamEventQueue};
pub use host_fence::{host_callback, host_callback_with};
pub use info::{MidiDialect, PluginCategory, PluginInfo};
pub use note_events::{
    NOTE_UNSPECIFIED, NoteBuffer, NoteEvent, NoteEventKind, NoteExpression, NoteTarget,
    append_notes_as_midi, route_param_mod, route_param_value,
};
pub use note_voices::{NoteVoice, NoteVoiceTable};
pub use plugin::{NoteNameEntry, PluginLogic};
pub use preset::{FactoryPreset, FactoryPresetState, apply_factory_preset, load_preset_file};
pub use process::{ProcessContext, ProcessStatus};
pub use state::{decode_state, encode_state};
pub use transport::Transport;
pub use tuning::{Tuning, TuningEvent, TuningInfo, TuningProvider};

// MIDI types live in `aura-midi`; re-export for process-path convenience.
pub use aura_midi::{
    MidiBuffer, MidiEvent, MidiMessage, MidiStatus, Ump, UmpBuffer, UmpEvent, append_midi_as_ump,
    append_ump_as_midi,
};
