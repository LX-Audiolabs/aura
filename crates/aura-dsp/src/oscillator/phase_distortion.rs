//! Phase-distortion / vector-synthesis oscillator.
//!
//! Provides a phase-distortion oscillator with two-dimensional shape control
//! (`shape_x`, `shape_y`) suitable for vector-synthesis morphing. The base
//! phasor is distorted using a CZ-style mapping, then four corner waveforms
//! (sine, cosine, saw, square) are blended according to the 2-D shape vector.

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
            sample_rate,
            shape_x: 0.0,
            shape_y: 0.0,
        }
    }

    pub fn set_shape_x(&mut self, value: f32) {
        self.shape_x = value.clamp(0.0, 1.0);
    }

    pub fn set_shape_y(&mut self, value: f32) {
        self.shape_y = value.clamp(0.0, 1.0);
    }

    fn advance(&mut self, frequency: f32) -> f32 {
        let phase_inc = frequency / self.sample_rate;
        self.phase += phase_inc;
        self.phase -= self.phase.floor();
        self.phase
    }

    fn distort_phase(phase: f32, shape: f32) -> f32 {
        // shape 0 -> sine-like, shape 1 -> saw-like
        let s = shape.clamp(0.0, 1.0);
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
        let shape_pairs = [
            (0.0, 0.0),
            (1.0, 0.0),
            (0.0, 1.0),
            (1.0, 1.0),
            (0.5, 0.5),
        ];
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
        let shape_pairs = [
            (0.0, 0.0),
            (1.0, 0.0),
            (0.0, 1.0),
            (1.0, 1.0),
            (0.5, 0.5),
        ];
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
}
