//! Wavetable synthesis with morphing support.
//!
//! Provides wavetable oscillators with linear interpolation and
//! the ability to morph between multiple wavetables.

use serde::{Deserialize, Serialize};

use crate::error::{self, NaadError, Result};

/// A single wavetable containing one cycle of a waveform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wavetable {
    /// The waveform samples (one cycle).
    samples: Vec<f32>,
}

impl Wavetable {
    /// Create a wavetable from raw samples.
    ///
    /// # Errors
    ///
    /// Returns `NaadError::InvalidParameter` if samples is empty.
    pub fn from_samples(samples: Vec<f32>) -> Result<Self> {
        if samples.is_empty() {
            return Err(NaadError::InvalidParameter {
                name: "samples".to_string(),
                reason: "wavetable must have at least one sample".to_string(),
            });
        }
        Ok(Self { samples })
    }

    /// Create a wavetable from additive harmonics.
    ///
    /// Generates a wavetable of `size` samples by summing sine waves at
    /// integer multiples of the fundamental, weighted by `amplitudes`.
    ///
    /// # Errors
    ///
    /// Returns `NaadError::InvalidParameter` if `num_harmonics` is 0,
    /// `amplitudes` is empty, or `size` is 0.
    pub fn from_harmonics(num_harmonics: usize, amplitudes: &[f32], size: usize) -> Result<Self> {
        if num_harmonics == 0 {
            return Err(NaadError::InvalidParameter {
                name: "num_harmonics".to_string(),
                reason: "must be > 0".to_string(),
            });
        }
        if amplitudes.is_empty() {
            return Err(NaadError::InvalidParameter {
                name: "amplitudes".to_string(),
                reason: "must not be empty".to_string(),
            });
        }
        if size == 0 {
            return Err(NaadError::InvalidParameter {
                name: "size".to_string(),
                reason: "must be > 0".to_string(),
            });
        }

        let mut samples = vec![0.0f32; size];
        let harmonics_to_generate = num_harmonics.min(amplitudes.len());

        for (h, &amp) in amplitudes.iter().take(harmonics_to_generate).enumerate() {
            let harmonic_num = (h + 1) as f32;
            for (i, sample) in samples.iter_mut().enumerate() {
                let phase = (i as f32 / size as f32) * std::f32::consts::TAU * harmonic_num;
                *sample += amp * phase.sin();
            }
        }

        // Normalize to -1..1
        let max_abs = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        if max_abs > 0.0 {
            for sample in &mut samples {
                *sample /= max_abs;
            }
        }

        Ok(Self { samples })
    }

    /// Returns a shared reference to the waveform samples.
    #[inline]
    #[must_use]
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    /// Returns the number of samples in the wavetable.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Returns `true` if the wavetable contains no samples.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Read a sample from the wavetable with linear interpolation.
    #[inline]
    #[must_use]
    pub fn read_interpolated(&self, phase: f32) -> f32 {
        let len = self.samples.len() as f32;
        let index = phase * len;
        let index_floor = index.floor();
        let frac = index - index_floor;

        let i0 = (index_floor as usize) % self.samples.len();
        let i1 = (i0 + 1) % self.samples.len();

        self.samples[i0] * (1.0 - frac) + self.samples[i1] * frac
    }
}

/// Wavetable oscillator that reads from a single wavetable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WavetableOscillator {
    /// The wavetable to read from.
    table: Wavetable,
    /// Current phase (0.0 to 1.0).
    phase: f32,
    /// Sample rate in Hz.
    sample_rate: f32,
    /// Playback frequency in Hz.
    frequency: f32,
}

impl WavetableOscillator {
    /// Create a new wavetable oscillator.
    ///
    /// # Errors
    ///
    /// Returns error if sample_rate or frequency is invalid.
    pub fn new(table: Wavetable, frequency: f32, sample_rate: f32) -> Result<Self> {
        if let Some(e) = error::validate_sample_rate(sample_rate) {
            return Err(e);
        }
        if let Some(e) = error::validate_frequency(frequency, sample_rate) {
            return Err(e);
        }
        Ok(Self {
            table,
            phase: 0.0,
            sample_rate,
            frequency,
        })
    }

    /// Returns a shared reference to the wavetable.
    #[inline]
    #[must_use]
    pub fn table(&self) -> &Wavetable {
        &self.table
    }

    /// Returns the current phase (0.0 to 1.0).
    #[inline]
    #[must_use]
    pub fn phase(&self) -> f32 {
        self.phase
    }

    /// Returns the sample rate in Hz.
    #[inline]
    #[must_use]
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Returns the playback frequency in Hz.
    #[inline]
    #[must_use]
    pub fn frequency(&self) -> f32 {
        self.frequency
    }

    /// Set the playback frequency.
    ///
    /// # Errors
    ///
    /// Returns error if frequency is invalid for the current sample rate.
    pub fn set_frequency(&mut self, freq: f32) -> crate::error::Result<()> {
        if let Some(e) = error::validate_frequency(freq, self.sample_rate) {
            return Err(e);
        }
        self.frequency = freq;
        Ok(())
    }

    /// Generate the next sample with linear interpolation.
    #[inline]
    #[must_use]
    pub fn next_sample(&mut self) -> f32 {
        let sample = self.table.read_interpolated(self.phase);

        self.phase += self.frequency / self.sample_rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        sample
    }

    /// Fill a buffer with wavetable samples.
    #[inline]
    pub fn fill_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.next_sample();
        }
    }
}

/// A collection of wavetables that can be morphed between.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MorphWavetable {
    /// The wavetables to morph between.
    tables: Vec<Wavetable>,
    /// Morph position (0.0 to 1.0).
    position: f32,
    /// Current phase (0.0 to 1.0).
    phase: f32,
    /// Sample rate in Hz.
    sample_rate: f32,
    /// Playback frequency in Hz.
    frequency: f32,
}

impl MorphWavetable {
    /// Create a new morph wavetable.
    ///
    /// # Errors
    ///
    /// Returns error if tables is empty, or sample_rate/frequency is invalid.
    pub fn new(tables: Vec<Wavetable>, frequency: f32, sample_rate: f32) -> Result<Self> {
        if tables.is_empty() {
            return Err(NaadError::InvalidParameter {
                name: "tables".to_string(),
                reason: "must have at least one wavetable".to_string(),
            });
        }
        // Validate all tables have the same size for correct morphing
        let first_len = tables[0].samples.len();
        if tables.iter().any(|t| t.samples.len() != first_len) {
            return Err(NaadError::InvalidParameter {
                name: "tables".to_string(),
                reason: "all wavetables must have the same number of samples".to_string(),
            });
        }
        if let Some(e) = error::validate_sample_rate(sample_rate) {
            return Err(e);
        }
        if let Some(e) = error::validate_frequency(frequency, sample_rate) {
            return Err(e);
        }
        Ok(Self {
            tables,
            position: 0.0,
            phase: 0.0,
            sample_rate,
            frequency,
        })
    }

    /// Returns a shared reference to the wavetables.
    #[inline]
    #[must_use]
    pub fn tables(&self) -> &[Wavetable] {
        &self.tables
    }

    /// Returns the current morph position (0.0 to 1.0).
    #[inline]
    #[must_use]
    pub fn position(&self) -> f32 {
        self.position
    }

    /// Returns the current phase (0.0 to 1.0).
    #[inline]
    #[must_use]
    pub fn phase(&self) -> f32 {
        self.phase
    }

    /// Returns the sample rate in Hz.
    #[inline]
    #[must_use]
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Returns the playback frequency in Hz.
    #[inline]
    #[must_use]
    pub fn frequency(&self) -> f32 {
        self.frequency
    }

    /// Set the morph position (clamped to 0.0..1.0).
    pub fn set_morph(&mut self, position: f32) {
        self.position = position.clamp(0.0, 1.0);
    }

    /// Generate the next sample, interpolating between wavetables.
    #[inline]
    #[must_use]
    pub fn next_sample(&mut self) -> f32 {
        let num_tables = self.tables.len();
        let sample = if num_tables == 1 {
            self.tables[0].read_interpolated(self.phase)
        } else {
            let scaled = self.position * (num_tables - 1) as f32;
            let idx_low = (scaled.floor() as usize).min(num_tables - 2);
            let idx_high = idx_low + 1;
            let frac = scaled - idx_low as f32;

            let s_low = self.tables[idx_low].read_interpolated(self.phase);
            let s_high = self.tables[idx_high].read_interpolated(self.phase);
            s_low * (1.0 - frac) + s_high * frac
        };

        self.phase += self.frequency / self.sample_rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        sample
    }

    /// Generate the next sample with C²-smooth (cubic B-spline) morphing across tables.
    ///
    /// Where [`Self::next_sample`] linearly blends the two adjacent tables,
    /// `next_sample_smooth` evaluates a clamped cubic B-spline through *all*
    /// tables (sampled at the current phase) and reads it at `position`.
    /// This eliminates the audible "kink" as the morph parameter sweeps past
    /// table boundaries — the C²-continuous curve has matched first and
    /// second derivatives across each table transition.
    ///
    /// Falls back to [`Self::next_sample`]'s linear blend when fewer than 4
    /// tables are present (cubic B-spline requires at least 4 control
    /// points). Slower than `next_sample` — call only when the smoother
    /// morph is worth the cost.
    ///
    /// Requires the `synthesis` feature (uses hisab B-spline).
    #[cfg(feature = "synthesis")]
    #[inline]
    #[must_use]
    pub fn next_sample_smooth(&mut self) -> f32 {
        if self.tables.len() < 4 {
            return self.next_sample();
        }

        // Sample every table at the current phase, then evaluate the
        // B-spline across those scalar control points at `position`.
        let cps: Vec<f32> = self
            .tables
            .iter()
            .map(|t| t.read_interpolated(self.phase))
            .collect();
        let sample =
            crate::dsp_util::bspline_eval_1d(3, &cps, self.position).unwrap_or_else(|| {
                // Defensive fallback — bspline_eval should not fail given our
                // construction-time invariants, but degrade gracefully if it does.
                let scaled = self.position * (self.tables.len() - 1) as f32;
                let idx_low = (scaled.floor() as usize).min(self.tables.len() - 2);
                let idx_high = idx_low + 1;
                let frac = scaled - idx_low as f32;
                cps[idx_low] * (1.0 - frac) + cps[idx_high] * frac
            });

        self.phase += self.frequency / self.sample_rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        sample
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_samples() {
        let wt = Wavetable::from_samples(vec![0.0, 1.0, 0.0, -1.0]).unwrap();
        assert_eq!(wt.len(), 4);
    }

    #[test]
    fn test_from_samples_empty() {
        assert!(Wavetable::from_samples(vec![]).is_err());
    }

    #[test]
    fn test_from_harmonics() {
        let wt = Wavetable::from_harmonics(3, &[1.0, 0.5, 0.25], 1024).unwrap();
        assert_eq!(wt.len(), 1024);
        let max = wt.samples().iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!((max - 1.0).abs() < 0.01, "should be normalized to 1.0");
    }

    #[test]
    fn test_interpolated_read() {
        let wt = Wavetable::from_samples(vec![0.0, 1.0, 0.0, -1.0]).unwrap();
        let s = wt.read_interpolated(0.125); // between index 0 and 1
        assert!(s > 0.0 && s < 1.0);
    }

    #[test]
    fn test_wavetable_oscillator() {
        let wt = Wavetable::from_harmonics(1, &[1.0], 256).unwrap();
        let mut osc = WavetableOscillator::new(wt, 440.0, 44100.0).unwrap();
        let mut buf = [0.0f32; 256];
        osc.fill_buffer(&mut buf);
        assert!(buf.iter().any(|&s| s != 0.0));
    }

    #[test]
    fn test_morph_wavetable() {
        let wt1 = Wavetable::from_harmonics(1, &[1.0], 256).unwrap();
        let wt2 = Wavetable::from_harmonics(2, &[1.0, 0.5], 256).unwrap();
        let mut morph = MorphWavetable::new(vec![wt1, wt2], 440.0, 44100.0).unwrap();
        morph.set_morph(0.5);
        let s = morph.next_sample();
        assert!(s.is_finite());
    }

    #[test]
    fn test_serde_roundtrip() {
        let wt = Wavetable::from_samples(vec![0.0, 1.0, 0.0, -1.0]).unwrap();
        let json = serde_json::to_string(&wt).unwrap();
        let back: Wavetable = serde_json::from_str(&json).unwrap();
        assert_eq!(wt.samples(), back.samples());
    }

    #[cfg(feature = "synthesis")]
    fn morph_with_n_tables(n: usize) -> MorphWavetable {
        let tables: Vec<Wavetable> = (0..n)
            .map(|h| {
                let amps: Vec<f32> = (0..(h + 1)).map(|_| 1.0 / (h + 1) as f32).collect();
                Wavetable::from_harmonics(h + 1, &amps, 256).unwrap()
            })
            .collect();
        MorphWavetable::new(tables, 440.0, 44100.0).unwrap()
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_morph_smooth_falls_back_with_few_tables() {
        // <4 tables → smooth() falls back to linear next_sample().
        let mut m1 = morph_with_n_tables(2);
        let mut m2 = morph_with_n_tables(2);
        m1.set_morph(0.5);
        m2.set_morph(0.5);
        let s1 = m1.next_sample();
        let s2 = m2.next_sample_smooth();
        assert!(
            (s1 - s2).abs() < 1e-5,
            "with 2 tables, smooth should equal linear; got {s1} vs {s2}"
        );
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_morph_smooth_differs_from_linear_with_4_tables() {
        // ≥4 tables → smooth path actually engages the B-spline.
        let mut linear = morph_with_n_tables(5);
        let mut smooth = morph_with_n_tables(5);
        linear.set_morph(0.4);
        smooth.set_morph(0.4);

        let mut total_diff = 0.0f32;
        for _ in 0..256 {
            let l = linear.next_sample();
            let s = smooth.next_sample_smooth();
            total_diff += (l - s).abs();
        }
        // Different curves through the same control points → outputs should
        // diverge meaningfully across a buffer.
        assert!(
            total_diff > 0.1,
            "linear vs smooth must differ across a buffer; total |diff| = {total_diff}"
        );
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_morph_smooth_finite_at_boundaries() {
        let mut m = morph_with_n_tables(5);
        for &pos in &[0.0f32, 0.5, 1.0] {
            m.set_morph(pos);
            for _ in 0..32 {
                let s = m.next_sample_smooth();
                assert!(s.is_finite(), "pos={pos}: got non-finite sample {s}");
            }
        }
    }
}
