//! Voice management for polyphonic synthesis.
//!
//! Provides voice allocation, stealing, and per-voice state tracking
//! for polyphonic instruments.

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

/// Inline voice capacity — the SmallVec spills to the heap above this.
/// 16 covers the vast majority of polyphonic-synth use cases without alloc.
const VOICE_INLINE: usize = 16;

/// Polyphony mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PolyMode {
    /// Polyphonic — each note gets its own voice.
    Poly,
    /// Monophonic — only one note at a time, retrigger on new note.
    Mono,
    /// Legato — monophonic but glides to new note without retriggering.
    Legato,
}

/// Voice stealing strategy when all voices are in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum StealMode {
    /// Steal the oldest active voice.
    Oldest,
    /// Steal the quietest active voice.
    Quietest,
    /// Steal the voice with the lowest note.
    Lowest,
    /// Do not steal — ignore new notes when full.
    None,
}

/// State of a single voice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Voice {
    /// Whether this voice is currently active (producing sound).
    active: bool,
    /// The MIDI note number this voice is playing (0-127).
    pub note: u8,
    /// Velocity (0.0 to 1.0).
    pub velocity: f32,
    /// Age counter — incremented each sample when active. Used for oldest-steal.
    age: u64,
    /// Current amplitude (for quietest-steal heuristic).
    pub amplitude: f32,
    /// Per-note pitch bend (MIDI 2.0, in semitones).
    pub pitch_bend: f32,
    /// Per-note pressure / aftertouch (0.0 to 1.0).
    pub pressure: f32,
    /// Per-note brightness (MIDI 2.0 CC74, 0.0 to 1.0).
    pub brightness: f32,
}

impl Voice {
    /// Returns whether this voice is currently active (producing sound).
    #[inline]
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Returns the age counter for this voice.
    #[inline]
    #[must_use]
    pub fn age(&self) -> u64 {
        self.age
    }
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            active: false,
            note: 0,
            velocity: 0.0,
            age: 0,
            amplitude: 0.0,
            pitch_bend: 0.0,
            pressure: 0.0,
            brightness: 0.5,
        }
    }
}

/// Manages voice allocation and stealing for polyphonic instruments.
///
/// The voice pool lives in a [`SmallVec`] with `VOICE_INLINE = 16` slots
/// inline — typical polysynth configurations (4–16 voices) stay entirely
/// stack-resident, while larger pools (up to 128) spill to the heap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceManager {
    /// Voice pool.
    pub(crate) voices: SmallVec<[Voice; VOICE_INLINE]>,
    /// Maximum polyphony.
    max_voices: usize,
    /// Polyphony mode.
    pub poly_mode: PolyMode,
    /// Voice stealing mode.
    pub steal_mode: StealMode,
}

impl VoiceManager {
    /// Create a new voice manager.
    ///
    /// `max_voices` is clamped to 1..128.
    #[must_use]
    pub fn new(max_voices: usize, poly_mode: PolyMode, steal_mode: StealMode) -> Self {
        let n = max_voices.clamp(1, 128);
        Self {
            voices: (0..n).map(|_| Voice::default()).collect(),
            max_voices: n,
            poly_mode,
            steal_mode,
        }
    }

    /// Allocate a voice for a new note. Returns the voice index.
    ///
    /// In Poly mode: finds a free voice or steals one.
    /// In Mono/Legato mode: always uses voice 0.
    #[must_use]
    pub fn note_on(&mut self, note: u8, velocity: f32) -> Option<usize> {
        match self.poly_mode {
            PolyMode::Mono | PolyMode::Legato => {
                let v = &mut self.voices[0];
                v.active = true;
                v.note = note;
                v.velocity = velocity;
                v.age = 0;
                Some(0)
            }
            PolyMode::Poly => {
                // First: find a free voice
                if let Some(idx) = self.voices.iter().position(|v| !v.active) {
                    let v = &mut self.voices[idx];
                    v.active = true;
                    v.note = note;
                    v.velocity = velocity;
                    v.age = 0;
                    return Some(idx);
                }

                // All voices busy — steal
                let idx = self.find_steal_target()?;
                let v = &mut self.voices[idx];
                v.active = true;
                v.note = note;
                v.velocity = velocity;
                v.age = 0;
                Some(idx)
            }
        }
    }

    /// Release a voice by note number.
    ///
    /// Returns the index of the released voice, or `None` if not found.
    pub fn note_off(&mut self, note: u8) -> Option<usize> {
        if let Some(idx) = self.voices.iter().position(|v| v.active && v.note == note) {
            self.voices[idx].active = false;
            Some(idx)
        } else {
            None
        }
    }

    /// Find a voice to steal based on the current steal mode.
    fn find_steal_target(&self) -> Option<usize> {
        match self.steal_mode {
            StealMode::None => None,
            StealMode::Oldest => self
                .voices
                .iter()
                .enumerate()
                .filter(|(_, v)| v.active)
                .max_by_key(|(_, v)| v.age)
                .map(|(i, _)| i),
            StealMode::Quietest => self
                .voices
                .iter()
                .enumerate()
                .filter(|(_, v)| v.active)
                .min_by(|(_, a), (_, b)| {
                    a.amplitude
                        .partial_cmp(&b.amplitude)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i),
            StealMode::Lowest => self
                .voices
                .iter()
                .enumerate()
                .filter(|(_, v)| v.active)
                .min_by_key(|(_, v)| v.note)
                .map(|(i, _)| i),
        }
    }

    /// Advance age counters for all active voices (call once per sample or per buffer).
    pub fn tick(&mut self) {
        for v in &mut self.voices {
            if v.active {
                v.age = v.age.saturating_add(1);
            }
        }
    }

    /// Returns the number of currently active voices.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    /// Returns a shared reference to the voice pool.
    #[inline]
    #[must_use]
    pub fn voices(&self) -> &[Voice] {
        &self.voices
    }

    /// Returns a mutable reference to a voice by index, or `None` if out of bounds.
    #[inline]
    pub fn voice_mut(&mut self, index: usize) -> Option<&mut Voice> {
        self.voices.get_mut(index)
    }

    /// Returns the maximum polyphony.
    #[must_use]
    pub fn max_voices(&self) -> usize {
        self.max_voices
    }

    /// Release all voices.
    pub fn all_notes_off(&mut self) {
        for v in &mut self.voices {
            v.active = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_on_off() {
        let mut vm = VoiceManager::new(4, PolyMode::Poly, StealMode::Oldest);
        let idx = vm.note_on(60, 0.8).unwrap();
        assert_eq!(vm.active_count(), 1);
        assert_eq!(vm.voices()[idx].note, 60);
        vm.note_off(60);
        assert_eq!(vm.active_count(), 0);
    }

    #[test]
    fn test_poly_fill_and_steal() {
        let mut vm = VoiceManager::new(2, PolyMode::Poly, StealMode::Oldest);
        let _ = vm.note_on(60, 0.8);
        vm.tick();
        let _ = vm.note_on(64, 0.7);
        vm.tick();
        assert_eq!(vm.active_count(), 2);
        // Third note should steal oldest
        let idx = vm.note_on(67, 0.9).unwrap();
        assert_eq!(vm.voices()[idx].note, 67);
        assert_eq!(vm.active_count(), 2);
    }

    #[test]
    fn test_steal_none() {
        let mut vm = VoiceManager::new(1, PolyMode::Poly, StealMode::None);
        let _ = vm.note_on(60, 0.8);
        assert!(vm.note_on(64, 0.7).is_none());
    }

    #[test]
    fn test_mono_mode() {
        let mut vm = VoiceManager::new(4, PolyMode::Mono, StealMode::Oldest);
        let _ = vm.note_on(60, 0.8);
        let _ = vm.note_on(64, 0.7);
        // Mono always uses voice 0
        assert_eq!(vm.voices()[0].note, 64);
        assert_eq!(vm.active_count(), 1);
    }

    #[test]
    fn test_all_notes_off() {
        let mut vm = VoiceManager::new(4, PolyMode::Poly, StealMode::Oldest);
        let _ = vm.note_on(60, 0.8);
        let _ = vm.note_on(64, 0.7);
        vm.all_notes_off();
        assert_eq!(vm.active_count(), 0);
    }

    #[test]
    fn test_serde_roundtrip() {
        let mut vm = VoiceManager::new(4, PolyMode::Poly, StealMode::Oldest);
        let _ = vm.note_on(60, 0.8);
        let json = serde_json::to_string(&vm).unwrap();
        let back: VoiceManager = serde_json::from_str(&json).unwrap();
        assert_eq!(vm.active_count(), back.active_count());
        assert_eq!(vm.max_voices(), back.max_voices());
    }
}
