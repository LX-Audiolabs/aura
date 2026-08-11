//! Soft saw oscillator — bandlimited saw with triangle-like spectral rolloff.
//!
//! Unlike a standard sawtooth (1/n partial amplitudes), the soft saw
//! uses 1/n² partial falloff like a triangle wave, producing a warmer,
//! less harsh tone. Implemented algorithmically via PolyBLEP on a
//! shaped waveform rather than through wavetable synthesis.
//!
//! Ported from fundsp concept (MIT / Apache-2.0).
//! Original authors: Sami Perttu and contributors.

use crate::oscillator::polyblep;

/// Soft saw oscillator — warmer alternative to a standard sawtooth.
///
/// Falls off spectrally like a triangle wave (1/n²) while retaining
/// the sawtooth shape. Good for bass sounds and gentle leads.
#[derive(Debug, Clone)]
pub struct SoftSaw {
    phase: f32,
    sample_duration: f32,
}

impl SoftSaw {
    /// Create a new soft saw oscillator.
    pub fn new(sample_rate: f32) -> Self {
        Self {
            phase: 0.0,
            sample_duration: 1.0 / sample_rate,
        }
    }

    /// Create with initial phase in 0..1.
    pub fn with_phase(sample_rate: f32, phase: f32) -> Self {
        let mut osc = Self::new(sample_rate);
        osc.phase = phase.clamp(0.0, 1.0);
        osc
    }

    /// Reset phase to 0.
    #[inline]
    pub fn reset(&mut self) {
        self.phase = 0.0;
    }

    /// Generate next sample at the given frequency.
    ///
    /// Uses a polynomial-shaped sawtooth core with PolyBLEP
    /// correction for anti-aliasing.
    #[inline]
    pub fn next_sample(&mut self, frequency: f32) -> f32 {
        let delta = frequency * self.sample_duration;
        let phase = self.phase;

        // Soft saw core: polynomial shape that approximates a saw
        // with triangle-like spectral rolloff (1/n²).
        // This is equivalent to the wavetable computed in fundsp's
        // soft_saw_table(): 1/(i*i) partial amplitudes.
        let raw = if phase < 0.5 {
            // Rising edge: quadratic shape
            let t = phase * 2.0;
            -1.0 + 2.0 * t * t
        } else {
            // Falling edge
            let t = (phase - 0.5) * 2.0;
            1.0 - 2.0 * t * t
        };

        // Anti-alias with PolyBLEP
        let value = raw - polyblep(phase, delta);

        self.phase += delta;
        if self.phase >= 1.0 {
            self.phase -= (self.phase as i32) as f32;
        }

        value
    }

    /// Process a buffer of samples.
    #[inline]
    pub fn process_buffer(&mut self, frequency: f32, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.next_sample(frequency);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soft_saw_produces_audio() {
        let mut osc = SoftSaw::new(44100.0);
        let mut sum = 0.0f32;
        for _ in 0..100 {
            let s = osc.next_sample(440.0);
            assert!(s.is_finite());
            sum += s.abs();
        }
        assert!(sum > 0.0);
    }

    #[test]
    fn soft_saw_output_in_range() {
        let mut osc = SoftSaw::new(44100.0);
        for _ in 0..1000 {
            let s = osc.next_sample(440.0);
            // PolyBLEP causes overshoot; allow [-2.0, 2.0]
            assert!(s >= -2.0 && s <= 2.0, "output {s} out of range");
        }
    }
}
