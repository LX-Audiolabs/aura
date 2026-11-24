//! Phase-distortion / vector-synthesis oscillator.
//!
//! Provides a phase-distortion oscillator with two-dimensional shape control
//! (`shape_x`, `shape_y`) suitable for vector-synthesis morphing. The base
//! phasor is distorted using a CZ-style mapping, then four corner waveforms
//! (sine, cosine, saw, square) are blended according to the 2-D shape vector.

/// Triangle wave-folder: maps any finite input into `[-1, 1]` by reflecting at
/// the `±1` boundaries. `drive` scales the input as `sample * (1 + drive)`;
/// higher values produce more folds.
#[inline]
#[must_use]
pub fn wavefold(sample: f32, drive: f32) -> f32 {
    if !sample.is_finite() || !drive.is_finite() {
        return 0.0;
    }
    let driven = f64::from(sample) * (1.0 + f64::from(drive.max(0.0)));
    (1.0 - ((driven + 1.0).rem_euclid(4.0) - 2.0).abs()) as f32
}

#[derive(Debug, Clone)]
pub struct PhaseDistortionOscillator {
    phase: f32,
    sample_rate: f32,
    shape_x: f32,
    shape_y: f32,
}

impl PhaseDistortionOscillator {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            phase: 0.0,
            sample_rate: if sample_rate.is_finite() && sample_rate > 0.0 {
                sample_rate
            } else {
                48_000.0
            },
            shape_x: 0.0,
            shape_y: 0.0,
        }
    }

    pub fn set_shape_x(&mut self, value: f32) {
        self.shape_x = if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.0
        };
    }

    pub fn set_shape_y(&mut self, value: f32) {
        self.shape_y = if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.0
        };
    }

    fn advance(&mut self, frequency: f32) -> f32 {
        let phase_inc = if frequency.is_finite() {
            f64::from(frequency) / f64::from(self.sample_rate)
        } else {
            0.0
        };
        self.phase = (f64::from(self.phase) + phase_inc).rem_euclid(1.0) as f32;
        self.phase
    }

    fn distort_phase(phase: f32, shape: f32) -> f32 {
        // shape 0 -> sine-like, shape 1 -> saw-like
        // Neutral shape is identity; keep the knee away from singular endpoints.
        let s = 0.5 - 0.49 * shape.clamp(0.0, 1.0);
        if phase < s {
            0.5 * phase / s.max(0.0001)
        } else {
            0.5 + 0.5 * (phase - s) / (1.0 - s).max(0.0001)
        }
    }

    fn vector_morph(&self, phase: f32) -> f32 {
        let sine = (phase * std::f32::consts::TAU).sin();
        let cosine = (phase * std::f32::consts::TAU).cos();
        let saw = 2.0 * phase - 1.0;
        let square = if phase < 0.5 { 1.0 } else { -1.0 };

        let top = sine * (1.0 - self.shape_x) + saw * self.shape_x;
        let bottom = cosine * (1.0 - self.shape_x) + square * self.shape_x;

        top * (1.0 - self.shape_y) + bottom * self.shape_y
    }

    pub fn next_sample(&mut self, frequency: f32) -> f32 {
        let phase = self.advance(frequency);
        let distorted = Self::distort_phase(phase, self.shape_x);
        self.vector_morph(distorted)
    }

    /// Generate the next sample with optional wave-folder drive applied.
    ///
    /// `drive` scales the oscillator output before folding; `0.0` leaves the
    /// amplitude unchanged, while larger values produce more aggressive folding.
    #[inline]
    #[must_use]
    pub fn next_sample_with_drive(&mut self, frequency: f32, drive: f32) -> f32 {
        wavefold(self.next_sample(frequency), drive)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_initializes_fields() {
        let osc = PhaseDistortionOscillator::new(48_000.0);
        assert_eq!(osc.phase, 0.0);
        assert_eq!(osc.sample_rate, 48_000.0);
        assert_eq!(osc.shape_x, 0.0);
        assert_eq!(osc.shape_y, 0.0);
    }

    #[test]
    fn shape_setters_clamp() {
        let mut osc = PhaseDistortionOscillator::new(48_000.0);
        osc.set_shape_x(1.5);
        osc.set_shape_y(-0.5);
        assert_eq!(osc.shape_x, 1.0);
        assert_eq!(osc.shape_y, 0.0);
    }

    #[test]
    fn advance_wraps_phase() {
        let mut osc = PhaseDistortionOscillator::new(10.0);
        // At sample_rate 10 Hz, a frequency of 25 Hz advances 2.5 phases per sample.
        let phase = osc.advance(25.0);
        assert!((phase - 0.5).abs() < 1e-6, "phase was {phase}");
    }

    #[test]
    fn distort_phase_bounds() {
        for i in 0..=10 {
            let phase = i as f32 / 10.0;
            let out = PhaseDistortionOscillator::distort_phase(phase, 0.3);
            assert!((0.0..=1.0).contains(&out), "distorted {out} out of bounds");
        }
    }

    #[test]
    fn vector_morph_bounds() {
        let osc = PhaseDistortionOscillator::new(48_000.0);
        for i in 0..=10 {
            let phase = i as f32 / 10.0;
            let out = osc.vector_morph(phase);
            assert!(out.abs() <= 1.0, "morph output {out} out of bounds");
        }
    }

    #[test]
    fn next_sample_produces_finite_output() {
        let mut osc = PhaseDistortionOscillator::new(48_000.0);
        for _ in 0..100 {
            let sample = osc.next_sample(440.0);
            assert!(sample.is_finite(), "non-finite sample {sample}");
            assert!(sample.abs() <= 1.0, "sample {sample} out of bounds");
        }
    }

    #[test]
    fn distort_phase_sweeps_shape() {
        for shape in [0.0, 0.5, 1.0] {
            for i in 0..=10 {
                let phase = i as f32 / 10.0;
                let out = PhaseDistortionOscillator::distort_phase(phase, shape);
                assert!(
                    (0.0..=1.0).contains(&out),
                    "distorted {out} out of bounds at shape {shape}, phase {phase}"
                );
            }
        }
    }

    #[test]
    fn vector_morph_sweeps_shape_axes() {
        let shape_pairs = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0), (0.5, 0.5)];
        for (shape_x, shape_y) in shape_pairs {
            let mut osc = PhaseDistortionOscillator::new(48_000.0);
            osc.set_shape_x(shape_x);
            osc.set_shape_y(shape_y);
            for i in 0..=10 {
                let phase = i as f32 / 10.0;
                let out = osc.vector_morph(phase);
                assert!(
                    out.abs() <= 1.0,
                    "morph output {out} out of bounds at ({shape_x}, {shape_y}), phase {phase}"
                );
            }
        }
    }

    #[test]
    fn next_sample_sweeps_shape_axes() {
        let shape_pairs = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0), (0.5, 0.5)];
        for (shape_x, shape_y) in shape_pairs {
            let mut osc = PhaseDistortionOscillator::new(48_000.0);
            osc.set_shape_x(shape_x);
            osc.set_shape_y(shape_y);
            for _ in 0..100 {
                let sample = osc.next_sample(440.0);
                assert!(
                    sample.is_finite(),
                    "non-finite sample {sample} at ({shape_x}, {shape_y})"
                );
                assert!(
                    sample.abs() <= 1.0,
                    "sample {sample} out of bounds at ({shape_x}, {shape_y})"
                );
            }
        }
    }

    #[test]
    fn wavefold_known_values() {
        for (input, expected) in [
            (0.0, 0.0),
            (1.0, 1.0),
            (2.0, 0.0),
            (3.0, -1.0),
            (4.0, 0.0),
            (-1.0, -1.0),
            (-2.0, 0.0),
        ] {
            assert!((wavefold(input, 0.0) - expected).abs() < 1e-6);
        }

        // Drive doubles the effective input.
        assert!((wavefold(0.5, 1.0) - wavefold(1.0, 0.0)).abs() < 1e-6);
        assert!((wavefold(1.0, 2.0) - wavefold(3.0, 0.0)).abs() < 1e-6);
    }

    #[test]
    fn wavefold_output_in_bounds() {
        for i in -20..=20 {
            let sample = i as f32 * 0.25;
            for drive in [0.0, 0.5, 1.0, 3.0, 10.0] {
                let out = wavefold(sample, drive);
                assert!(
                    out.abs() <= 1.0,
                    "wavefold({sample}, {drive}) = {out} out of bounds"
                );
            }
        }
    }

    #[test]
    fn wavefold_periodicity() {
        let drive = 2.0;
        let period = 4.0 / (1.0 + drive);
        for i in -5..=5 {
            let sample = i as f32 * 0.3;
            let a = wavefold(sample, drive);
            let b = wavefold(sample + period, drive);
            assert!(
                (a - b).abs() < 1e-5,
                "wavefold not periodic at sample {sample}, drive {drive}: {a} vs {b}"
            );
        }
    }

    #[test]
    fn next_sample_with_drive_is_finite_and_bounded() {
        let mut osc = PhaseDistortionOscillator::new(48_000.0);
        for _ in 0..100 {
            let sample = osc.next_sample_with_drive(440.0, 2.0);
            assert!(sample.is_finite(), "non-finite driven sample {sample}");
            assert!(sample.abs() <= 1.0, "driven sample {sample} out of bounds");
        }
    }

    #[test]
    fn next_sample_with_drive_sweeps_shape_axes() {
        let shape_pairs = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0), (0.5, 0.5)];
        for (shape_x, shape_y) in shape_pairs {
            let mut osc = PhaseDistortionOscillator::new(48_000.0);
            osc.set_shape_x(shape_x);
            osc.set_shape_y(shape_y);
            for _ in 0..100 {
                let sample = osc.next_sample_with_drive(440.0, 1.5);
                assert!(
                    sample.is_finite(),
                    "non-finite driven sample {sample} at ({shape_x}, {shape_y})"
                );
                assert!(
                    sample.abs() <= 1.0,
                    "driven sample {sample} out of bounds at ({shape_x}, {shape_y})"
                );
            }
        }
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;

    #[test]
    fn regression_neutral_sine() {
        let mut o = PhaseDistortionOscillator::new(48_000.0);
        for i in 1..=48 {
            let expected = (std::f32::consts::TAU * (i % 48) as f32 / 48.0).sin();
            assert!((o.next_sample(1000.0) - expected).abs() < 1e-5);
        }
    }
    #[test]
    fn regression_wavefold_unity() {
        for i in -20..=20 {
            let x = i as f32 / 20.0;
            assert!((wavefold(x, 0.0) - x).abs() < 1e-6);
            assert!((wavefold(x, 3.0) + wavefold(-x, 3.0)).abs() < 1e-6);
        }
    }
}
