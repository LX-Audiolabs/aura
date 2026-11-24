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

/// Octave-spaced mip levels, richest first. Construct before audio processing.
static SOFT_SAW_TABLES: LazyLock<[Wavetable; 7]> = LazyLock::new(|| {
    std::array::from_fn(|level| {
        let harmonics = 64 >> level;
        let amps: Vec<f32> = (1..=harmonics).map(|n| 1.0 / (n * n) as f32).collect();
        Wavetable::from_harmonics(harmonics, &amps, 2048).expect("soft-saw wavetable")
    })
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
    tables: &'static [Wavetable; 7],
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
            tables: LazyLock::force(&SOFT_SAW_TABLES),
        })
    }

    /// Create with initial phase in 0…1.
    ///
    /// # Errors
    ///
    /// Returns error if `sample_rate` is invalid.
    pub fn with_phase(sample_rate: f32, phase: f32) -> Result<Self> {
        let mut osc = Self::new(sample_rate)?;
        osc.phase = if phase.is_finite() {
            phase.rem_euclid(1.0)
        } else {
            0.0
        };
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
        if !frequency.is_finite() || frequency <= 0.0 || frequency >= self.sample_rate * 0.5 {
            return 0.0;
        }
        let budget = f64::from(self.sample_rate) * 0.5 / f64::from(frequency);
        let limit = budget.ceil() - 1.0;
        let level = (0..7)
            .find(|level| f64::from(64 >> level) <= limit)
            .unwrap_or(6);
        let rich = self.tables[level].read_interpolated(self.phase);
        let lean = if level < 6 {
            self.tables[level + 1].read_interpolated(self.phase)
        } else {
            0.0
        };
        // Fade a level in only after all its harmonics fit below Nyquist.
        let blend = (budget / f64::from(64 >> level) - 1.0).clamp(0.0, 1.0) as f32;
        let sample = lean + (rich - lean) * blend;
        self.phase = (self.phase + frequency / self.sample_rate).rem_euclid(1.0);
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
        let osc = SoftSaw::new(48_000.0).unwrap();
        assert!(osc.tables.iter().all(|table| table.len() >= 1024));
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;
    #[test]
    fn regression_high_note_alias() {
        let mut o = SoftSaw::new(48_000.0).unwrap();
        let mut re = 0.0_f64;
        let mut im = 0.0_f64;
        for i in 0..4800 {
            let y = f64::from(o.next_sample(10_000.0));
            let t = std::f64::consts::TAU * 18_000.0 * f64::from(i) / 48_000.0;
            re += y * t.cos();
            im += y * t.sin();
        }
        assert!(2.0 * re.hypot(im) / 4800.0 < 1e-4);
    }
}

#[cfg(test)]
mod continuity_tests {
    use super::*;
    #[test]
    fn mip_boundaries_are_continuous() {
        for harmonic in [2.0, 4.0, 8.0, 16.0, 32.0, 64.0] {
            let freq = 24_000.0 / harmonic;
            let mut below = SoftSaw::with_phase(48_000.0, 0.13).unwrap();
            let mut above = below.clone();
            assert!(
                (below.next_sample(freq * 0.99999) - above.next_sample(freq * 1.00001)).abs()
                    < 1e-3
            );
        }
    }
}
