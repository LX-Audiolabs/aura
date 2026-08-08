//! Tuning and pitch utilities.
//!
//! Provides equal temperament, MIDI note conversion, cent calculations,
//! and custom tuning tables (just intonation, Pythagorean, etc.).

use serde::{Deserialize, Serialize};

use crate::error::{NaadError, Result};

/// Calculate the frequency of a note in 12-tone equal temperament.
///
/// Uses the formula: `a4_hz * 2^((note - 69) / 12)`.
/// Note 69 = A4 = 440 Hz (standard tuning).
#[inline]
#[must_use]
pub fn equal_temperament_freq(note: u8, a4_hz: f32) -> f32 {
    a4_hz * 2.0f32.powf((note as f32 - 69.0) / 12.0)
}

/// Convert a MIDI note number to frequency in Hz (A4 = 440 Hz).
#[inline]
#[must_use]
pub fn midi_to_freq(note: u8) -> f32 {
    equal_temperament_freq(note, 440.0)
}

/// Convert a frequency in Hz to the nearest MIDI note number.
///
/// Returns the MIDI note number (0-127) closest to the given frequency.
/// The result is clamped to the valid MIDI range.
#[inline]
#[must_use]
pub fn freq_to_midi(freq: f32) -> u8 {
    if freq <= 0.0 {
        return 0;
    }
    let note = 69.0 + 12.0 * (freq / 440.0).log2();
    note.round().clamp(0.0, 127.0) as u8
}

/// Calculate the interval in cents between two frequencies.
///
/// Cents = 1200 * log2(f2 / f1).
/// 100 cents = 1 semitone in equal temperament.
#[inline]
#[must_use]
pub fn cents(f1: f32, f2: f32) -> f32 {
    if f1 <= 0.0 || f2 <= 0.0 {
        return 0.0;
    }
    1200.0 * (f2 / f1).log2()
}

/// Predefined tuning system types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum TuningSystem {
    /// Standard 12-tone equal temperament.
    EqualTemperament,
    /// Just intonation (pure intervals based on small integer ratios).
    JustIntonation,
    /// Pythagorean tuning (based on perfect fifths, ratio 3:2).
    Pythagorean,
}

/// Custom tuning table with 12 pitch ratios per octave.
///
/// Each entry represents the ratio from the root note for that
/// scale degree within a single octave.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuningTable {
    /// Name of this tuning.
    pub name: String,
    /// 12 ratios (one per semitone within an octave).
    /// Index 0 = unison (1.0), index 11 = major seventh.
    pub ratios: [f32; 12],
    /// Reference frequency for A4.
    pub a4_hz: f32,
}

impl TuningTable {
    /// Create a tuning table from a predefined tuning system.
    #[must_use]
    pub fn from_system(system: TuningSystem, a4_hz: f32) -> Self {
        let (name, ratios) = match system {
            TuningSystem::EqualTemperament => {
                let mut r = [0.0f32; 12];
                for (i, ratio) in r.iter_mut().enumerate() {
                    *ratio = 2.0f32.powf(i as f32 / 12.0);
                }
                ("Equal Temperament".to_string(), r)
            }
            TuningSystem::JustIntonation => (
                "Just Intonation".to_string(),
                [
                    1.0,         // Unison
                    16.0 / 15.0, // Minor second
                    9.0 / 8.0,   // Major second
                    6.0 / 5.0,   // Minor third
                    5.0 / 4.0,   // Major third
                    4.0 / 3.0,   // Perfect fourth
                    45.0 / 32.0, // Tritone
                    3.0 / 2.0,   // Perfect fifth
                    8.0 / 5.0,   // Minor sixth
                    5.0 / 3.0,   // Major sixth
                    9.0 / 5.0,   // Minor seventh
                    15.0 / 8.0,  // Major seventh
                ],
            ),
            TuningSystem::Pythagorean => (
                "Pythagorean".to_string(),
                [
                    1.0,           // Unison
                    256.0 / 243.0, // Minor second
                    9.0 / 8.0,     // Major second
                    32.0 / 27.0,   // Minor third
                    81.0 / 64.0,   // Major third
                    4.0 / 3.0,     // Perfect fourth
                    729.0 / 512.0, // Tritone
                    3.0 / 2.0,     // Perfect fifth
                    128.0 / 81.0,  // Minor sixth
                    27.0 / 16.0,   // Major sixth
                    16.0 / 9.0,    // Minor seventh
                    243.0 / 128.0, // Major seventh
                ],
            ),
        };

        Self {
            name,
            ratios,
            a4_hz,
        }
    }

    /// Create a custom tuning table.
    ///
    /// # Errors
    ///
    /// Returns error if any ratio is <= 0 or a4_hz is invalid.
    pub fn custom(name: String, ratios: [f32; 12], a4_hz: f32) -> Result<Self> {
        if a4_hz <= 0.0 || !a4_hz.is_finite() {
            return Err(NaadError::InvalidParameter {
                name: "a4_hz".to_string(),
                reason: "must be > 0 and finite".to_string(),
            });
        }
        for (i, &r) in ratios.iter().enumerate() {
            if r <= 0.0 || !r.is_finite() {
                return Err(NaadError::InvalidParameter {
                    name: format!("ratios[{i}]"),
                    reason: "must be > 0 and finite".to_string(),
                });
            }
        }
        Ok(Self {
            name,
            ratios,
            a4_hz,
        })
    }

    /// Get the frequency for a MIDI note number using this tuning table.
    ///
    /// Note 69 = A4. The octave is determined by integer division,
    /// and the semitone within the octave selects the ratio.
    #[must_use]
    pub fn note_to_freq(&self, note: u8) -> f32 {
        let note_i = note as i32;
        // A4 = note 69, which is degree 9 in the octave (A is 9 semitones above C)
        let a4_degree = 9;
        let semitone_from_a4 = note_i - 69;

        // Find the octave offset and degree
        let total_degree = a4_degree + semitone_from_a4;
        let octave = total_degree.div_euclid(12);
        let degree = total_degree.rem_euclid(12) as usize;

        let ratio = self.ratios[degree];
        // A4's own ratio in the table is ratios[9] (A is degree 9)
        let a4_ratio = self.ratios[a4_degree as usize];

        self.a4_hz * (ratio / a4_ratio) * 2.0f32.powi(octave - a4_degree.div_euclid(12))
    }
}

/// Note name helper — convert a MIDI note to a name string.
#[must_use]
pub fn note_name(note: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let octave = (note / 12) as i32 - 1;
    let name = NAMES[(note % 12) as usize];
    format!("{name}{octave}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_a4_440() {
        let freq = midi_to_freq(69);
        assert!(
            (freq - 440.0).abs() < 0.01,
            "A4 should be 440 Hz, got {freq}"
        );
    }

    #[test]
    fn test_c4_261() {
        let freq = midi_to_freq(60);
        assert!(
            (freq - 261.63).abs() < 0.1,
            "C4 should be ~261.63 Hz, got {freq}"
        );
    }

    #[test]
    fn test_octave() {
        let a4 = midi_to_freq(69);
        let a5 = midi_to_freq(81);
        assert!((a5 / a4 - 2.0).abs() < 0.01, "octave should be 2:1 ratio");
    }

    #[test]
    fn test_freq_to_midi_roundtrip() {
        for note in 21..=108 {
            let freq = midi_to_freq(note);
            let back = freq_to_midi(freq);
            assert_eq!(note, back, "roundtrip failed for note {note}");
        }
    }

    #[test]
    fn test_cents() {
        let c = cents(440.0, 880.0);
        assert!(
            (c - 1200.0).abs() < 0.1,
            "octave should be 1200 cents, got {c}"
        );
    }

    #[test]
    fn test_cents_semitone() {
        let a4 = midi_to_freq(69);
        let bb4 = midi_to_freq(70);
        let c = cents(a4, bb4);
        assert!(
            (c - 100.0).abs() < 0.1,
            "semitone should be 100 cents, got {c}"
        );
    }

    #[test]
    fn test_note_name() {
        assert_eq!(note_name(69), "A4");
        assert_eq!(note_name(60), "C4");
        assert_eq!(note_name(72), "C5");
    }

    #[test]
    fn test_just_intonation() {
        let table = TuningTable::from_system(TuningSystem::JustIntonation, 440.0);
        let a4 = table.note_to_freq(69);
        assert!(
            (a4 - 440.0).abs() < 0.1,
            "A4 in just intonation should be 440 Hz, got {a4}"
        );
    }

    #[test]
    fn test_pythagorean() {
        let table = TuningTable::from_system(TuningSystem::Pythagorean, 440.0);
        let a4 = table.note_to_freq(69);
        assert!(
            (a4 - 440.0).abs() < 0.1,
            "A4 in Pythagorean should be 440 Hz, got {a4}"
        );
    }

    #[test]
    fn test_equal_temperament_table() {
        let table = TuningTable::from_system(TuningSystem::EqualTemperament, 440.0);
        let c4 = table.note_to_freq(60);
        let et_c4 = midi_to_freq(60);
        assert!(
            (c4 - et_c4).abs() < 1.0,
            "ET table C4={c4} should match midi_to_freq C4={et_c4}"
        );
    }

    #[test]
    fn test_custom_tuning_validation() {
        let mut ratios = [1.0f32; 12];
        ratios[3] = -1.0;
        assert!(TuningTable::custom("bad".to_string(), ratios, 440.0).is_err());
    }

    #[test]
    fn test_serde_roundtrip() {
        let table = TuningTable::from_system(TuningSystem::JustIntonation, 442.0);
        let json = serde_json::to_string(&table).unwrap();
        let back: TuningTable = serde_json::from_str(&json).unwrap();
        assert_eq!(table.name, back.name);
        assert!((table.a4_hz - back.a4_hz).abs() < f32::EPSILON);
    }
}
