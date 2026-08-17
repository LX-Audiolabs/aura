//! Per-block process context (minimal).

use aura_midi::MidiBuffer;

use crate::config::ProcessMode;
use crate::note_events::NoteBuffer;

/// What the plugin wants after `process`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ProcessStatus {
    /// Keep calling process (normal).
    #[default]
    Continue,
    /// No more output expected until something changes (tail finished).
    TailFinished,
    /// Error — host may suspend.
    Error,
}

/// Per-block context handed to `process()`.
///
/// Format wrappers fill [`Self::midi`] from host note/MIDI events (CLAP first;
/// VST3/LV2 empty until wired). Plugins read sample-accurate messages there.
///
/// CLAP also fills [`Self::notes`] with native note-on/off/choke, note
/// expressions, and per-note `PARAM_MOD` (`note_id >= 0`). MIDI stays 7-bit.
///
/// Plugins that generate or pass through MIDI events push them into
/// [`Self::midi_out`]; wrappers flush them to the host after `process`.
#[non_exhaustive]
pub struct ProcessContext {
    pub sample_rate: f64,
    pub block_size: usize,
    pub process_mode: ProcessMode,
    /// Host timeline for this block; `None` when the host provides none.
    pub transport: Option<crate::transport::Transport>,
    /// Host → plugin MIDI / note events for this block (sorted by sample offset).
    pub midi: MidiBuffer,
    /// Plugin → host MIDI / note events for this block (sorted by sample offset).
    pub midi_out: MidiBuffer,
    /// CLAP-shaped notes / expressions / poly mods (empty on VST3/LV2 today).
    pub notes: NoteBuffer,
}

impl ProcessContext {
    #[must_use]
    pub fn new(sample_rate: f64, block_size: usize) -> Self {
        Self {
            sample_rate,
            block_size,
            process_mode: ProcessMode::Realtime,
            transport: None,
            midi: MidiBuffer::new(),
            midi_out: MidiBuffer::new(),
            notes: NoteBuffer::new(),
        }
    }

    #[must_use]
    pub fn with_process_mode(mut self, mode: ProcessMode) -> Self {
        self.process_mode = mode;
        self
    }

    #[must_use]
    pub fn with_transport(mut self, transport: crate::transport::Transport) -> Self {
        self.transport = Some(transport);
        self
    }

    #[must_use]
    pub fn with_midi(mut self, midi: MidiBuffer) -> Self {
        self.midi = midi;
        self
    }

    #[must_use]
    pub fn with_midi_out(mut self, midi_out: MidiBuffer) -> Self {
        self.midi_out = midi_out;
        self
    }

    #[must_use]
    pub fn with_notes(mut self, notes: NoteBuffer) -> Self {
        self.notes = notes;
        self
    }

    /// Clear both MIDI buffers so the context can be reused across blocks.
    pub fn clear_midi(&mut self) {
        self.midi.clear();
        self.midi_out.clear();
    }
}
