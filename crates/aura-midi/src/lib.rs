//! **aura-midi** — MIDI messages and block buffers.
//!
//! JUCE analogue: `juce_audio_basics/midi` (`MidiMessage`, `MidiBuffer`).
//! Format wrappers (CLAP/VST3/LV2) translate host events into these types
//! and pass them into process later (wired via `aura-core` when note events land).
//!
//! DSP (oscillators, voices, filters) lives in **`aura-dsp`**, not here.
//! Param host-learn hints stay in **`aura-params::MidiSource`**.
//!
//! MIDI 2.0: [`Ump`] encodes Universal MIDI Packets. `ProcessContext` still
//! carries 7-bit [`MidiMessage`]; wrappers down-convert `CLAP_EVENT_MIDI2`.

#![forbid(unsafe_code)]

mod buffer;
mod message;
mod note;
mod ump;

pub use buffer::{MidiBuffer, MidiEvent};
pub use message::{MidiMessage, MidiStatus};
pub use note::{midi_note_to_freq, midi_note_to_freq_a4, note_name};
pub use ump::Ump;
