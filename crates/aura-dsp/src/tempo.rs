//! Tempo / beat math for synced clocks and sequencers.
//!
//! AURA supplies only the math. Products map their own division indices
//! (e.g. Helm's fixed division table) to a `beats_per_cycle` value.

/// Hertz from tempo. `beats_per_cycle` is the musical length of one full
/// cycle (1.0 = one quarter-note cycle at `bpm`).
///
/// Non-finite or non-positive `bpm`, or non-finite / ≤0 `beats_per_cycle`,
/// returns `0.0` (stopped clock). No alloc.
#[inline]
#[must_use]
pub fn hz_from_tempo(bpm: f32, beats_per_cycle: f32) -> f32 {
    if !bpm.is_finite() || bpm <= 0.0 || !beats_per_cycle.is_finite() || beats_per_cycle <= 0.0 {
        0.0
    } else {
        bpm / 60.0 / beats_per_cycle
    }
}

/// Map song position in beats to a unit phase in `[0, 1)`.
///
/// Non-finite inputs or non-finite / ≤0 `beats_per_cycle` return `0.0`.
/// Uses `rem_euclid` so negative positions wrap.
#[inline]
#[must_use]
pub fn phase_from_beats(position_beats: f64, beats_per_cycle: f64) -> f32 {
    if !position_beats.is_finite() || !beats_per_cycle.is_finite() || beats_per_cycle <= 0.0 {
        0.0
    } else {
        (position_beats / beats_per_cycle).rem_euclid(1.0) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hz_from_tempo_120_bpm_one_beat_is_2_hz() {
        let hz = hz_from_tempo(120.0, 1.0);
        assert!((hz - 2.0).abs() < 1e-6, "got {hz}");
    }

    #[test]
    fn hz_from_tempo_120_bpm_half_beat_is_4_hz() {
        let hz = hz_from_tempo(120.0, 0.5);
        assert!((hz - 4.0).abs() < 1e-6, "got {hz}");
    }

    #[test]
    fn hz_from_tempo_non_finite_or_non_positive_is_zero() {
        for (bpm, bpc) in [
            (f32::NAN, 1.0),
            (f32::INFINITY, 1.0),
            (-120.0, 1.0),
            (0.0, 1.0),
            (120.0, 0.0),
            (120.0, -1.0),
            (120.0, f32::NAN),
        ] {
            let hz = hz_from_tempo(bpm, bpc);
            assert_eq!(hz, 0.0, "bpm={bpm:?} bpc={bpc:?}");
        }
    }

    #[test]
    fn phase_from_beats_wraps_unit_interval() {
        assert!((phase_from_beats(0.0, 4.0) - 0.0).abs() < 1e-6);
        assert!((phase_from_beats(2.0, 4.0) - 0.5).abs() < 1e-6);
        assert!((phase_from_beats(4.0, 4.0) - 0.0).abs() < 1e-6);
        assert!((phase_from_beats(5.0, 4.0) - 0.25).abs() < 1e-6);
        let neg = phase_from_beats(-1.0, 4.0);
        assert!((neg - 0.75).abs() < 1e-6, "got {neg}");
    }

    #[test]
    fn phase_from_beats_bad_inputs_zero() {
        assert_eq!(phase_from_beats(f64::NAN, 4.0), 0.0);
        assert_eq!(phase_from_beats(1.0, 0.0), 0.0);
        assert_eq!(phase_from_beats(1.0, f64::NAN), 0.0);
    }
}
