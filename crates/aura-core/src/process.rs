//! Per-block process context (minimal).

use aura_midi::{MidiBuffer, UmpBuffer};

use crate::config::ProcessMode;
use crate::note_events::NoteBuffer;
use crate::tuning::Tuning;

/// What the plugin wants after `process`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ProcessStatus {
    /// Keep calling process (normal).
    #[default]
    Continue,
    /// Let the host determine further processing from the reported tail length.
    /// CLAP maps this to `CLAP_PROCESS_TAIL`, not sleep.
    TailFinished,
    /// Error — host may suspend.
    Error,
    /// No further processing needed until an event (note, parameter, transport)
    /// or a change in audio input. The host may suspend processing until then.
    /// Backends without sleep support treat this as [`Self::Continue`].
    Sleep,
}

/// Per-block context handed to `process()`.
///
/// Format wrappers fill [`Self::midi`] (7-bit) and [`Self::ump`] (native
/// packets). CLAP writes `CLAP_EVENT_MIDI2` into `ump` unchanged and still
/// mirrors a MIDI 1 image into `midi` when one exists. VST3/LV2 lift 7-bit
/// MIDI into type-0x2 UMP so a plugin can read one buffer.
///
/// CLAP also fills [`Self::notes`] with native note-on/off/choke, note
/// expressions, and per-note `PARAM_MOD` (`note_id >= 0`).
///
/// Plugin → host: [`Self::midi_out`] (7-bit), [`Self::ump_out`] (native UMP;
/// CLAP emits `CLAP_EVENT_MIDI2`), [`Self::notes_out`] (`NOTE_*` / `NOTE_END`).
/// VST3/LV2 down-convert `ump_out` and On/Off/Choke.
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
    /// Plugin → host CLAP notes (`NOTE_ON`/`OFF`/`CHOKE`/`END` / expressions).
    pub notes_out: NoteBuffer,
    /// Host → plugin Universal MIDI Packets (MIDI 2 native + MIDI 1 as type 0x2).
    pub ump: UmpBuffer,
    /// Plugin → host UMP. CLAP emits `CLAP_EVENT_MIDI2`; VST3/LV2 down-convert.
    pub ump_out: UmpBuffer,
    /// Host-driven tuning context (CLAP `clap.tuning`).
    pub tuning: Tuning,
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
            notes_out: NoteBuffer::new(),
            ump: UmpBuffer::new(),
            ump_out: UmpBuffer::new(),
            tuning: Tuning::disabled(),
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

    #[must_use]
    pub fn with_notes_out(mut self, notes_out: NoteBuffer) -> Self {
        self.notes_out = notes_out;
        self
    }

    #[must_use]
    pub fn with_ump(mut self, ump: UmpBuffer) -> Self {
        self.ump = ump;
        self
    }

    #[must_use]
    pub fn with_ump_out(mut self, ump_out: UmpBuffer) -> Self {
        self.ump_out = ump_out;
        self
    }

    #[must_use]
    pub fn with_tuning(mut self, tuning: Tuning) -> Self {
        self.tuning = tuning;
        self
    }

    /// Clear all event buffers so the context can be reused across blocks.
    pub fn clear_events(&mut self) {
        self.midi.clear();
        self.midi_out.clear();
        self.notes.clear();
        self.notes_out.clear();
        self.ump.clear();
        self.ump_out.clear();
    }
}

/// RAII guard that enables Flush-To-Zero and Denormals-Are-Zero on `x86_64`
/// for the duration of the current scope and restores the previous MXCSR
/// value when dropped. Thread-bound: create/drop on the processing thread.
/// On non-`x86_64` targets this is a no-op; the host/product must enable the
/// platform denormal mode once at its callback edge.
pub struct FtzDazGuard {
    #[cfg(target_arch = "x86_64")]
    old_csr: u32,
    thread: std::marker::PhantomData<*mut ()>,
}

impl FtzDazGuard {
    /// Capture the current MXCSR, set FTZ/DAZ, and return the guard.
    #[must_use]
    #[allow(deprecated)]
    pub fn new() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            // SAFETY: SSE/MXCSR is available on every x86_64 target.
            let old_csr = unsafe { std::arch::x86_64::_mm_getcsr() };
            // SAFETY: Only documented FTZ/DAZ bits are added to valid MXCSR.
            unsafe { std::arch::x86_64::_mm_setcsr(old_csr | 0x8040) };
            Self {
                old_csr,
                thread: std::marker::PhantomData,
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            Self {
                thread: std::marker::PhantomData,
            }
        }
    }
}

impl Default for FtzDazGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for FtzDazGuard {
    #[allow(deprecated)]
    fn drop(&mut self) {
        #[cfg(target_arch = "x86_64")]
        // SAFETY: Restore the saved MXCSR on the same thread.
        unsafe {
            std::arch::x86_64::_mm_setcsr(self.old_csr);
        }
    }
}

#[cfg(all(test, target_arch = "x86_64"))]
mod ftz_tests {
    #[test]
    #[allow(deprecated)]
    fn callback_guard_enables_ftz_daz_and_restores_caller_environment() {
        let saved = unsafe { std::arch::x86_64::_mm_getcsr() };
        unsafe { std::arch::x86_64::_mm_setcsr(saved & !0x8040) };
        {
            let _guard = super::FtzDazGuard::new();
            assert_eq!(unsafe { std::arch::x86_64::_mm_getcsr() } & 0x8040, 0x8040);
        }
        let restored = unsafe { std::arch::x86_64::_mm_getcsr() };
        unsafe { std::arch::x86_64::_mm_setcsr(saved) };
        assert_eq!(restored, saved & !0x8040);
    }
}
