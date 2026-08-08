//! **aura-midi** — MIDI messages and block buffers.
//!
//! JUCE analogue: `juce_audio_basics/midi` (`MidiMessage`, `MidiBuffer`).
//! Format wrappers (CLAP/VST3/LV2) translate host events into these types
//! and pass them into process later (wired via `aura-core` when note events land).
//!
//! DSP (oscillators, voices, filters) lives in **`aura-dsp`**, not here.
//! Param host-learn hints stay in **`aura-params::MidiSource`**.

#![forbid(unsafe_code)]

mod buffer;
mod message;
mod note;

pub use buffer::{MidiBuffer, MidiEvent};
pub use message::{MidiMessage, MidiStatus};
pub use note::{midi_note_to_freq, midi_note_to_freq_a4, note_name};
