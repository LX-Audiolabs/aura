//! Note number helpers (tuning tables stay in `aura-dsp::tuning`).

const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// Equal-temperament Hz for MIDI note with A4 = `a4_hz` (usually 440).
#[must_use]
pub fn midi_note_to_freq_a4(note: u8, a4_hz: f32) -> f32 {
    let n = f32::from(note);
    a4_hz * 2.0_f32.powf((n - 69.0) / 12.0)
}

/// Equal-temperament Hz, A4 = 440 Hz.
#[must_use]
pub fn midi_note_to_freq(note: u8) -> f32 {
    midi_note_to_freq_a4(note, 440.0)
}

/// Scientific pitch name, e.g. `60` → `"C4"`.
#[must_use]
pub fn note_name(note: u8) -> String {
    let name = NOTE_NAMES[usize::from(note % 12)];
    let octave = (i16::from(note) / 12) - 1;
    format!("{name}{octave}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a4_is_440() {
        assert!((midi_note_to_freq(69) - 440.0).abs() < 1e-4);
    }

    #[test]
    fn middle_c_name() {
        assert_eq!(note_name(60), "C4");
    }
}
