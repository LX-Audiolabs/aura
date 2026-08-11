//! Soft saw oscillator — bandlimited saw with triangle-like spectral rolloff.
//!
//! Ported from fundsp `soft_saw_table()` / wavetable (MIT / Apache-2.0).
//! Original authors: Sami Perttu and contributors.
//!
//! Unlike a classic saw (partial amplitudes `1/n`), the soft saw uses
//! `1/n²` falloff like a triangle, yielding a warmer, less harsh tone.
//! Implemented as an additive wavetable (same spectral idea as fundsp),
//! not as a PolyBLEP-shaped polynomial.

use std::sync::LazyLock;

use crate::error::{self, Result};
use crate::wavetable::Wavetable;

/// One cycle, additive soft-saw: harmonic `n` amplitude `1/n²`, n = 1…64.
static SOFT_SAW_TABLE: LazyLock<Wavetable> = LazyLock::new(|| {
    const N: usize = 64;
    const SIZE: usize = 2048;
    let amps: Vec<f32> = (1..=N)
        .map(|i| {
            let n = i as f32;
            1.0 / (n * n)
        })
        .collect();
    Wavetable::from_harmonics(N, &amps, SIZE).expect("soft-saw wavetable")
});

/// Soft saw oscillator — warmer alternative to a standard sawtooth.
///
/// Spectral envelope matches fundsp's soft-saw wavetable (`1/n²` partials).
/// Good for bass and gentle leads.
///
/// # Examples
///
/// ```rust
/// use aura_dsp::oscillator::SoftSaw;
/// let mut osc = SoftSaw::new(44100.0).unwrap();
/// let s = osc.next_sample(440.0);
/// assert!(s.is_finite());
/// ```
#[derive(Debug, Clone)]
pub struct SoftSaw {
    phase: f32,
    sample_rate: f32,
}

impl SoftSaw {
    /// Create a new soft saw oscillator.
    ///
    /// # Errors
    ///
    /// Returns error if `sample_rate` is invalid.
    pub fn new(sample_rate: f32) -> Result<Self> {
        if let Some(e) = error::validate_sample_rate(sample_rate) {
            return Err(e);
        }
        Ok(Self {
            phase: 0.0,
            sample_rate,
        })
    }

    /// Create with initial phase in 0…1.
    ///
    /// # Errors
    ///
    /// Returns error if `sample_rate` is invalid.
    pub fn with_phase(sample_rate: f32, phase: f32) -> Result<Self> {
        let mut osc = Self::new(sample_rate)?;
        osc.phase = phase.clamp(0.0, 1.0);
        Ok(osc)
    }

    /// Reset phase to 0.
    #[inline]
    pub fn reset(&mut self) {
        self.phase = 0.0;
    }

    /// Generate next sample at the given frequency (Hz).
    #[inline]
    pub fn next_sample(&mut self, frequency: f32) -> f32 {
        let sample = SOFT_SAW_TABLE.read_interpolated(self.phase);
        let delta = frequency.max(0.0) / self.sample_rate;
        self.phase += delta;
        self.phase -= self.phase.floor();
        sample
    }

    /// Process a buffer of samples at constant frequency.
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
        let mut osc = SoftSaw::new(44100.0).unwrap();
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
        let mut osc = SoftSaw::new(44100.0).unwrap();
        for _ in 0..1000 {
            let s = osc.next_sample(440.0);
            // Normalized wavetable stays in ~[-1, 1]
            assert!((-1.05..=1.05).contains(&s), "output {s} out of range");
        }
    }

    #[test]
    fn soft_saw_invalid_sr() {
        assert!(SoftSaw::new(0.0).is_err());
    }

    #[test]
    fn soft_saw_table_has_content() {
        assert!(!SOFT_SAW_TABLE.is_empty());
        assert!(SOFT_SAW_TABLE.len() >= 1024);
    }
}
