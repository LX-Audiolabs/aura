//! Additive synthesis with per-partial control.
//!
//! Builds complex timbres by summing individually controllable sine
//! partials. Each partial has an independent frequency ratio (relative
//! to a fundamental) and amplitude. The default constructor creates a
//! harmonic series with 1/n amplitude rolloff.

use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Maximum number of partials supported.
const MAX_PARTIALS: usize = 64;

/// One sine partial in an [`AdditiveSynth`] bank.
///
/// A partial is `amplitude * sin(2π * (fundamental * frequency_ratio) * t + phase)`.
/// `frequency_ratio` is relative to the synth's fundamental (e.g. 1.0 = unison,
/// 2.0 = octave above, 1.5 = perfect fifth). The `phase` field is an initial
/// offset in turns (0..1); the engine maintains the running phase separately.
/// Partials whose absolute frequency would exceed Nyquist are zeroed by the
/// engine to prevent aliasing.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Partial {
    /// Frequency multiplier relative to the fundamental.
    pub frequency_ratio: f32,
    /// Amplitude (0.0 to 1.0).
    pub amplitude: f32,
    /// Initial phase offset in turns (0..1).
    pub phase: f32,
}

/// Additive synthesis engine.
///
/// Sums up to 64 sine-wave partials, each at an independent frequency
/// ratio relative to the fundamental.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdditiveSynth {
    /// Fundamental frequency in Hz.
    fundamental: f32,
    /// Partial definitions.
    partials: Vec<Partial>,
    /// Running phases for each partial (0..1).
    phases: Vec<f32>,
    /// Sample rate in Hz.
    sample_rate: f32,
}

impl AdditiveSynth {
    /// Create a new additive synth with a harmonic series.
    ///
    /// Generates `num_partials` harmonics at 1x, 2x, 3x ... with
    /// amplitudes of 1/1, 1/2, 1/3 ... (normalized).
    ///
    /// # Errors
    ///
    /// Returns error if parameters are invalid.
    pub fn new(fundamental: f32, num_partials: usize, sample_rate: f32) -> Result<Self> {
        if sample_rate <= 0.0 || !sample_rate.is_finite() {
            return Err(crate::error::NaadError::InvalidSampleRate { sample_rate });
        }
        if fundamental <= 0.0 || !fundamental.is_finite() {
            return Err(crate::error::NaadError::InvalidFrequency {
                frequency: fundamental,
                nyquist: sample_rate / 2.0,
            });
        }

        let count = num_partials.clamp(1, MAX_PARTIALS);
        let mut partials = Vec::with_capacity(count);
        let nyquist = sample_rate / 2.0;

        for i in 0..count {
            let ratio = (i + 1) as f32;
            // Skip partials above Nyquist.
            let amp = if fundamental * ratio < nyquist {
                1.0 / ratio
            } else {
                0.0
            };
            partials.push(Partial {
                frequency_ratio: ratio,
                amplitude: amp,
                phase: 0.0,
            });
        }

        let phases = vec![0.0; count];

        Ok(Self {
            fundamental,
            partials,
            phases,
            sample_rate,
        })
    }

    /// Set the fundamental frequency.
    ///
    /// # Errors
    ///
    /// Returns error if frequency is invalid.
    pub fn set_fundamental(&mut self, freq: f32) -> Result<()> {
        if freq <= 0.0 || !freq.is_finite() {
            return Err(crate::error::NaadError::InvalidFrequency {
                frequency: freq,
                nyquist: self.sample_rate / 2.0,
            });
        }
        self.fundamental = freq;
        // Re-check Nyquist: zero out partials whose frequency now exceeds Nyquist
        let nyquist = self.sample_rate / 2.0;
        for p in &mut self.partials {
            if freq * p.frequency_ratio >= nyquist {
                p.amplitude = 0.0;
            }
        }
        Ok(())
    }

    /// Configure an individual partial by index.
    ///
    /// Amplitude is clamped to 0.0 if the partial frequency exceeds Nyquist.
    /// Does nothing if `index` is out of range.
    pub fn set_partial(&mut self, index: usize, freq_ratio: f32, amplitude: f32) {
        if let Some(p) = self.partials.get_mut(index) {
            p.frequency_ratio = freq_ratio;
            let nyquist = self.sample_rate / 2.0;
            if self.fundamental * freq_ratio >= nyquist {
                p.amplitude = 0.0;
            } else {
                p.amplitude = amplitude.clamp(0.0, 1.0);
            }
        }
    }

    /// Returns the number of partials.
    #[must_use]
    pub fn num_partials(&self) -> usize {
        self.partials.len()
    }

    /// Returns the fundamental frequency.
    #[must_use]
    pub fn fundamental(&self) -> f32 {
        self.fundamental
    }

    /// Generate the next output sample.
    #[inline]
    #[must_use]
    pub fn next_sample(&mut self) -> f32 {
        let mut sum = 0.0f32;

        for (i, partial) in self.partials.iter().enumerate() {
            if partial.amplitude <= 0.0 {
                continue;
            }
            let phase = self.phases[i];
            sum += (phase * std::f32::consts::TAU).sin() * partial.amplitude;

            let freq = self.fundamental * partial.frequency_ratio;
            let inc = freq / self.sample_rate;
            let new_phase = self.phases[i] + inc;
            self.phases[i] = new_phase - new_phase.floor();
        }

        sum
    }

    /// Fill a buffer with samples.
    #[inline]
    pub fn fill_buffer(&mut self, buffer: &mut [f32]) {
        for s in buffer.iter_mut() {
            *s = self.next_sample();
        }
    }

    /// Returns true if any partials have non-zero amplitude.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.partials.iter().any(|p| p.amplitude > 0.0)
    }

    /// Compress the partial amplitude bank via DCT-II, keeping the first `num_coeffs` coefficients.
    ///
    /// Smooth amplitude envelopes (typical for harmonic spectra — `1/n`
    /// rolloff, formant peaks, etc.) concentrate most of their energy in
    /// the low-order DCT coefficients, so truncating the spectrum gives
    /// efficient lossy compression for preset storage / transmission.
    /// Pair with [`Self::restore_amplitudes_dct`] to expand on the receiving
    /// side.
    ///
    /// `num_coeffs` is clamped to `[1, num_partials]`.
    ///
    /// # Errors
    ///
    /// Returns [`crate::NaadError::ComputationError`] if the underlying
    /// `hisab::num::dct` call fails.
    ///
    /// Requires the `synthesis` feature (uses hisab DCT).
    pub fn compress_amplitudes_dct(&self, num_coeffs: usize) -> Result<Vec<f64>> {
        let amps: Vec<f64> = self.partials.iter().map(|p| p.amplitude as f64).collect();
        let coeffs =
            hisab::num::dct(&amps).map_err(|e| crate::error::NaadError::ComputationError {
                message: format!("DCT failed: {e:?}"),
            })?;
        let keep = num_coeffs.clamp(1, coeffs.len());
        Ok(coeffs.into_iter().take(keep).collect())
    }

    /// Restore partial amplitudes from DCT coefficients (inverse of [`Self::compress_amplitudes_dct`]).
    ///
    /// Zero-pads `coeffs` out to the current partial count, runs the
    /// inverse DCT, and writes the result back into the partial bank
    /// (clamped to `[0.0, 1.0]` and re-Nyquist-filtered). Partial
    /// frequency ratios and phases are unchanged.
    ///
    /// # Errors
    ///
    /// Returns [`crate::NaadError::InvalidParameter`] if `coeffs` is
    /// empty or longer than the partial count, or
    /// [`crate::NaadError::ComputationError`] if `hisab::num::idct` fails.
    ///
    /// Requires the `synthesis` feature.
    pub fn restore_amplitudes_dct(&mut self, coeffs: &[f64]) -> Result<()> {
        let n = self.partials.len();
        if coeffs.is_empty() || coeffs.len() > n {
            return Err(crate::error::NaadError::InvalidParameter {
                name: "coeffs".to_string(),
                reason: format!("must be 1..={n} long, got {}", coeffs.len()),
            });
        }

        let mut padded = vec![0.0f64; n];
        padded[..coeffs.len()].copy_from_slice(coeffs);
        let restored =
            hisab::num::idct(&padded).map_err(|e| crate::error::NaadError::ComputationError {
                message: format!("IDCT failed: {e:?}"),
            })?;

        let nyquist = self.sample_rate / 2.0;
        for (p, &amp) in self.partials.iter_mut().zip(restored.iter()) {
            p.amplitude = if self.fundamental * p.frequency_ratio >= nyquist {
                0.0
            } else {
                (amp as f32).clamp(0.0, 1.0)
            };
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_partial_is_sine() {
        let mut synth = AdditiveSynth::new(440.0, 1, 44100.0).unwrap();
        let mut buf = [0.0f32; 1024];
        synth.fill_buffer(&mut buf);

        // Should produce a clean sine wave.
        assert!(
            buf.iter().any(|&s| s.abs() > 0.5),
            "single partial should produce sine output"
        );
        assert!(buf.iter().all(|s| s.is_finite()));

        // Verify approximate sine shape: max value should be close to 1.0
        // (the first partial has amplitude 1.0).
        let peak = buf.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!((peak - 1.0).abs() < 0.05, "peak should be near 1.0");
    }

    #[test]
    fn test_harmonic_series() {
        let mut synth = AdditiveSynth::new(440.0, 8, 44100.0).unwrap();
        let mut buf = [0.0f32; 1024];
        synth.fill_buffer(&mut buf);
        assert!(buf.iter().any(|&s| s.abs() > 0.5));
        assert!(buf.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn test_set_partial() {
        let mut synth = AdditiveSynth::new(440.0, 4, 44100.0).unwrap();
        synth.set_partial(0, 1.0, 0.0);
        synth.set_partial(1, 2.0, 1.0);

        let mut buf = [0.0f32; 1024];
        synth.fill_buffer(&mut buf);
        // Should still produce output from partial 1 (at 2x fundamental).
        assert!(buf.iter().any(|&s| s.abs() > 0.5));
    }

    #[test]
    fn test_serde_roundtrip() {
        let synth = AdditiveSynth::new(440.0, 16, 44100.0).unwrap();
        let json = serde_json::to_string(&synth).unwrap();
        let back: AdditiveSynth = serde_json::from_str(&json).unwrap();
        assert_eq!(synth.partials.len(), back.partials.len());
        assert!((synth.fundamental - back.fundamental).abs() < f32::EPSILON);
    }

    #[test]
    fn test_nyquist_filtering() {
        // At 8000 Hz sample rate, partials above 4000 Hz should be silent.
        let synth = AdditiveSynth::new(1000.0, 8, 8000.0).unwrap();
        // Partials 5-8 (5000, 6000, 7000, 8000 Hz) should have zero amplitude.
        for i in 4..synth.partials.len() {
            assert!(
                synth.partials[i].amplitude == 0.0,
                "partial {} should be zeroed (above Nyquist)",
                i + 1
            );
        }
    }

    #[test]
    fn test_dct_full_roundtrip_preserves_amplitudes() {
        // With all coefficients kept, DCT → IDCT should recover the
        // original amplitudes within numerical noise.
        let mut synth = AdditiveSynth::new(440.0, 16, 44100.0).unwrap();
        let original: Vec<f32> = synth.partials.iter().map(|p| p.amplitude).collect();

        let coeffs = synth.compress_amplitudes_dct(synth.num_partials()).unwrap();
        assert_eq!(coeffs.len(), 16);

        synth.restore_amplitudes_dct(&coeffs).unwrap();
        for (i, (orig, p)) in original.iter().zip(synth.partials.iter()).enumerate() {
            assert!(
                (orig - p.amplitude).abs() < 1e-4,
                "partial {i}: orig={orig}, restored={}",
                p.amplitude
            );
        }
    }

    #[test]
    fn test_dct_truncated_is_lossy_approximation() {
        // Keeping only the first 4 of 16 coefficients should reproduce
        // the smooth 1/n harmonic envelope reasonably (most energy is in
        // low-order DCT bins) but not exactly.
        let mut synth = AdditiveSynth::new(440.0, 16, 44100.0).unwrap();
        let original: Vec<f32> = synth.partials.iter().map(|p| p.amplitude).collect();

        let coeffs = synth.compress_amplitudes_dct(4).unwrap();
        assert_eq!(coeffs.len(), 4);
        synth.restore_amplitudes_dct(&coeffs).unwrap();

        // RMS error should be small relative to the amplitude scale.
        let mse: f32 = original
            .iter()
            .zip(synth.partials.iter())
            .map(|(o, p)| (o - p.amplitude).powi(2))
            .sum::<f32>()
            / original.len() as f32;
        let rmse = mse.sqrt();
        // 1/n harmonics compress reasonably — 4 of 16 coeffs holds RMSE < 0.15
        // (a 75% storage reduction at modest perceptual cost).
        assert!(rmse < 0.15, "truncated DCT roundtrip RMSE = {rmse}");
        // ...but not zero, since we discarded 12 coefficients.
        assert!(
            rmse > 1e-6,
            "with truncation RMSE shouldn't be exactly zero"
        );
    }

    #[test]
    fn test_dct_compress_clamps_num_coeffs() {
        let synth = AdditiveSynth::new(440.0, 8, 44100.0).unwrap();
        // num_coeffs > num_partials → clamps to num_partials
        let coeffs = synth.compress_amplitudes_dct(100).unwrap();
        assert_eq!(coeffs.len(), 8);
        // num_coeffs == 0 → clamps to 1
        let coeffs = synth.compress_amplitudes_dct(0).unwrap();
        assert_eq!(coeffs.len(), 1);
    }

    #[test]
    fn test_dct_restore_rejects_invalid_lengths() {
        let mut synth = AdditiveSynth::new(440.0, 8, 44100.0).unwrap();
        // empty
        assert!(synth.restore_amplitudes_dct(&[]).is_err());
        // too long
        assert!(synth.restore_amplitudes_dct(&vec![0.5; 99]).is_err());
    }

    #[test]
    fn test_dct_restore_respects_nyquist() {
        // Build a synth with partials above Nyquist (zeroed at construction),
        // then "restore" amplitudes that would re-enable them. The Nyquist
        // re-check should keep them silent.
        let mut synth = AdditiveSynth::new(1000.0, 8, 8000.0).unwrap();
        // Above-Nyquist partials are zeroed at construction.
        let pretend_full = vec![0.5f64; 8];
        // Run a DCT-IDCT cycle of the would-be amplitudes.
        let coeffs = hisab::num::dct(&pretend_full).unwrap();
        synth.restore_amplitudes_dct(&coeffs).unwrap();
        for (i, p) in synth.partials.iter().enumerate() {
            if 1000.0 * p.frequency_ratio >= 4000.0 {
                assert!(
                    p.amplitude == 0.0,
                    "partial {i} above Nyquist must remain 0.0 after restore"
                );
            }
        }
    }
}
