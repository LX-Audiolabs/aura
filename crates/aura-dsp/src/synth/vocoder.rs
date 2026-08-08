//! Channel vocoder.
//!
//! Splits both a modulator (voice) and carrier (synth) signal into
//! frequency bands using bandpass filters, extracts the envelope from
//! each modulator band, and applies it to the corresponding carrier
//! band. The result is the carrier signal shaped by the modulator's
//! spectral envelope — the classic "talking synth" effect.

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::error::Result;
use crate::filter::{BiquadFilter, FilterType};

/// Inline band capacity for the vocoder bank — typical configs use 8–16
/// bands and stay stack-resident; larger band counts spill to the heap.
const VOCODER_INLINE: usize = 16;

/// One frequency band of a [`Vocoder`].
///
/// Holds a parallel pair of bandpass filters — `analysis_filter` extracts
/// the modulator's energy in this band, `synthesis_filter` carves the same
/// band out of the carrier — plus an envelope follower (with separate
/// attack/release coefficients) that smooths the modulator level before
/// it amplitude-modulates the filtered carrier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VocoderBand {
    /// Bandpass filter for the modulator signal.
    analysis_filter: BiquadFilter,
    /// Bandpass filter for the carrier signal.
    synthesis_filter: BiquadFilter,
    /// Envelope follower state.
    envelope_follower: f32,
    /// Attack coefficient (smoothing for rising envelope).
    attack_coeff: f32,
    /// Release coefficient (smoothing for falling envelope).
    release_coeff: f32,
}

impl VocoderBand {
    /// Process one sample through this vocoder band.
    ///
    /// Returns the carrier filtered and amplitude-modulated by the
    /// modulator's envelope.
    #[inline]
    #[must_use]
    pub fn process(&mut self, modulator: f32, carrier: f32) -> f32 {
        // Bandpass the modulator and extract envelope.
        let mod_filtered = self.analysis_filter.process_sample(modulator);
        let mod_level = mod_filtered.abs();

        // Envelope follower with separate attack/release.
        let coeff = if mod_level > self.envelope_follower {
            self.attack_coeff
        } else {
            self.release_coeff
        };
        self.envelope_follower = crate::flush_denormal(
            self.envelope_follower + coeff * (mod_level - self.envelope_follower),
        );

        // Bandpass the carrier and apply the modulator envelope.
        let car_filtered = self.synthesis_filter.process_sample(carrier);
        car_filtered * self.envelope_follower
    }
}

/// Channel vocoder with configurable number of frequency bands.
///
/// Band storage is a [`SmallVec`] with `VOCODER_INLINE = 16` slots inline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vocoder {
    /// Frequency bands.
    bands: SmallVec<[VocoderBand; VOCODER_INLINE]>,
    /// Sample rate in Hz.
    sample_rate: f32,
}

impl Vocoder {
    /// Create a new vocoder with logarithmically-spaced bands.
    ///
    /// # Arguments
    ///
    /// * `num_bands` - Number of frequency bands (typically 8-16)
    /// * `low_freq` - Lowest band center frequency (Hz)
    /// * `high_freq` - Highest band center frequency (Hz)
    /// * `sample_rate` - Sample rate in Hz
    ///
    /// # Errors
    ///
    /// Returns error if any parameters are invalid.
    pub fn new(num_bands: usize, low_freq: f32, high_freq: f32, sample_rate: f32) -> Result<Self> {
        if sample_rate <= 0.0 || !sample_rate.is_finite() {
            return Err(crate::error::NaadError::InvalidSampleRate { sample_rate });
        }
        if num_bands == 0 {
            return Err(crate::error::NaadError::InvalidParameter {
                name: "num_bands".to_string(),
                reason: "must be > 0".to_string(),
            });
        }
        if low_freq <= 0.0 || high_freq <= low_freq {
            return Err(crate::error::NaadError::InvalidParameter {
                name: "frequency range".to_string(),
                reason: "low_freq must be > 0 and < high_freq".to_string(),
            });
        }

        let log_low = low_freq.ln();
        let log_high = high_freq.ln();
        let step = if num_bands > 1 {
            (log_high - log_low) / (num_bands - 1) as f32
        } else {
            0.0
        };

        // Envelope follower coefficients: ~5ms attack, ~20ms release.
        let attack_coeff = 1.0 - (-1.0 / (0.005 * sample_rate)).exp();
        let release_coeff = 1.0 - (-1.0 / (0.020 * sample_rate)).exp();

        let mut bands: SmallVec<[VocoderBand; VOCODER_INLINE]> = SmallVec::new();
        for i in 0..num_bands {
            let center = (log_low + step * i as f32).exp();
            // Clamp to Nyquist safety.
            let center = center.min(sample_rate * 0.49);
            // Q scales with band spacing for consistent bandwidth coverage.
            // For N logarithmically-spaced bands, Q ≈ 1/(exp(step)-1) ensures
            // adjacent bands overlap at their -3dB points.
            let q = if step > 0.0 {
                (1.0 / (step.exp() - 1.0)).clamp(1.0, 20.0)
            } else {
                4.0 // single band fallback
            };

            let analysis_filter = BiquadFilter::new(FilterType::BandPass, sample_rate, center, q)?;
            let synthesis_filter = BiquadFilter::new(FilterType::BandPass, sample_rate, center, q)?;

            bands.push(VocoderBand {
                analysis_filter,
                synthesis_filter,
                envelope_follower: 0.0,
                attack_coeff,
                release_coeff,
            });
        }

        Ok(Self { bands, sample_rate })
    }

    /// Process one sample pair (modulator, carrier) through all bands.
    ///
    /// Returns the sum of all band outputs.
    #[inline]
    #[must_use]
    pub fn process_sample(&mut self, modulator: f32, carrier: f32) -> f32 {
        let mut sum = 0.0f32;
        for band in &mut self.bands {
            sum += band.process(modulator, carrier);
        }
        sum
    }

    /// Process a buffer of modulator/carrier pairs.
    ///
    /// `modulator` and `carrier` must be the same length.
    /// Output is written to `output`.
    pub fn process_buffer(&mut self, modulator: &[f32], carrier: &[f32], output: &mut [f32]) {
        let len = modulator.len().min(carrier.len()).min(output.len());
        for i in 0..len {
            output[i] = self.process_sample(modulator[i], carrier[i]);
        }
    }

    /// Returns the number of bands.
    #[must_use]
    pub fn num_bands(&self) -> usize {
        self.bands.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_produces_output() {
        let mut vocoder = Vocoder::new(8, 200.0, 8000.0, 44100.0).unwrap();

        // Feed a sine as modulator and a saw-like signal as carrier.
        let mut has_output = false;
        for i in 0..2048 {
            let t = i as f32 / 44100.0;
            let modulator = (t * 440.0 * std::f32::consts::TAU).sin();
            let carrier = (t * 110.0 * std::f32::consts::TAU).sin();
            let out = vocoder.process_sample(modulator, carrier);
            assert!(out.is_finite());
            if out.abs() > 0.001 {
                has_output = true;
            }
        }
        assert!(has_output, "vocoder should produce output");
    }

    #[test]
    fn test_different_modulators_differ() {
        let mut vocoder1 = Vocoder::new(8, 200.0, 8000.0, 44100.0).unwrap();
        let mut vocoder2 = Vocoder::new(8, 200.0, 8000.0, 44100.0).unwrap();

        let mut out1 = Vec::with_capacity(1024);
        let mut out2 = Vec::with_capacity(1024);

        for i in 0..1024 {
            let t = i as f32 / 44100.0;
            let carrier = (t * 110.0 * std::f32::consts::TAU).sin();
            let mod1 = (t * 440.0 * std::f32::consts::TAU).sin();
            let mod2 = (t * 1200.0 * std::f32::consts::TAU).sin();
            out1.push(vocoder1.process_sample(mod1, carrier));
            out2.push(vocoder2.process_sample(mod2, carrier));
        }

        let diff: f32 = out1
            .iter()
            .zip(out2.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(
            diff > 0.01,
            "different modulators should produce different timbres"
        );
    }

    #[test]
    fn test_serde_roundtrip() {
        let vocoder = Vocoder::new(12, 100.0, 10000.0, 44100.0).unwrap();
        let json = serde_json::to_string(&vocoder).unwrap();
        let back: Vocoder = serde_json::from_str(&json).unwrap();
        assert_eq!(vocoder.bands.len(), back.bands.len());
    }

    #[test]
    fn test_process_buffer() {
        let mut vocoder = Vocoder::new(8, 200.0, 8000.0, 44100.0).unwrap();
        let modulator = vec![0.5; 64];
        let carrier = vec![0.3; 64];
        let mut output = vec![0.0; 64];
        vocoder.process_buffer(&modulator, &carrier, &mut output);
        assert!(output.iter().all(|s| s.is_finite()));
    }
}
