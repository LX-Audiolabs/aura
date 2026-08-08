//! Biquad and state variable filters.
//!
//! Implements the Audio EQ Cookbook (Robert Bristow-Johnson) formulas
//! for biquad filter coefficient computation, and a state variable filter
//! with simultaneous LP/HP/BP/Notch outputs.

use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::error::{self, NaadError, Result};

/// Filter type for biquad coefficient calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum FilterType {
    /// Low-pass filter.
    LowPass,
    /// High-pass filter.
    HighPass,
    /// Band-pass filter (constant skirt gain).
    BandPass,
    /// Notch (band-reject) filter.
    Notch,
    /// All-pass filter.
    AllPass,
    /// Low shelf filter.
    LowShelf,
    /// High shelf filter.
    HighShelf,
    /// Peaking EQ filter.
    Peak,
}

/// Biquad filter coefficients and state.
///
/// Uses Direct Form II Transposed implementation for better
/// numerical behavior with floating point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiquadFilter {
    // Coefficients (normalized by a0)
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    // State variables (Direct Form II Transposed)
    #[serde(skip)]
    z1: f32,
    #[serde(skip)]
    z2: f32,
    // Parameters for recalculation
    filter_type: FilterType,
    sample_rate: f32,
    frequency: f32,
    q: f32,
    gain_db: f32,
}

impl BiquadFilter {
    /// Create a new biquad filter.
    ///
    /// # Arguments
    ///
    /// * `filter_type` - Type of filter
    /// * `sample_rate` - Sample rate in Hz
    /// * `frequency` - Cutoff/center frequency in Hz
    /// * `q` - Q factor (resonance), must be > 0
    ///
    /// # Errors
    ///
    /// Returns error if frequency or sample_rate is invalid, or q <= 0.
    pub fn new(filter_type: FilterType, sample_rate: f32, frequency: f32, q: f32) -> Result<Self> {
        Self::with_gain(filter_type, sample_rate, frequency, q, 0.0)
    }

    /// Create a new biquad filter with gain (for shelf and peak filter types).
    ///
    /// # Arguments
    ///
    /// * `filter_type` - Type of filter
    /// * `sample_rate` - Sample rate in Hz
    /// * `frequency` - Cutoff/center frequency in Hz
    /// * `q` - Q factor (resonance), must be > 0
    /// * `gain_db` - Gain in dB (used by LowShelf, HighShelf, Peak)
    ///
    /// # Errors
    ///
    /// Returns error if frequency or sample_rate is invalid, or q <= 0.
    pub fn with_gain(
        filter_type: FilterType,
        sample_rate: f32,
        frequency: f32,
        q: f32,
        gain_db: f32,
    ) -> Result<Self> {
        if let Some(e) = error::validate_sample_rate(sample_rate) {
            return Err(e);
        }
        if let Some(e) = error::validate_frequency(frequency, sample_rate) {
            return Err(e);
        }
        if q <= 0.0 || !q.is_finite() {
            return Err(NaadError::InvalidParameter {
                name: "q".to_string(),
                reason: "must be > 0".to_string(),
            });
        }

        debug!(?filter_type, frequency, q, gain_db, "biquad filter created");
        let mut filter = Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: 0.0,
            z2: 0.0,
            filter_type,
            sample_rate,
            frequency,
            q,
            gain_db,
        };
        filter.compute_coefficients();
        Ok(filter)
    }

    /// Compute biquad coefficients from the Audio EQ Cookbook.
    fn compute_coefficients(&mut self) {
        let w0 = std::f32::consts::TAU * self.frequency / self.sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * self.q);

        let (b0, b1, b2, a0, a1, a2) = match self.filter_type {
            FilterType::LowPass => {
                let b1 = 1.0 - cos_w0;
                let b0 = b1 / 2.0;
                let b2 = b0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_w0;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::HighPass => {
                let b1_raw = 1.0 + cos_w0;
                let b0 = b1_raw / 2.0;
                let b1 = -(1.0 + cos_w0);
                let b2 = b0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_w0;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::BandPass => {
                let b0 = alpha;
                let b1 = 0.0;
                let b2 = -alpha;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_w0;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::Notch => {
                let b0 = 1.0;
                let b1 = -2.0 * cos_w0;
                let b2 = 1.0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_w0;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::AllPass => {
                let b0 = 1.0 - alpha;
                let b1 = -2.0 * cos_w0;
                let b2 = 1.0 + alpha;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_w0;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::LowShelf => {
                let a_gain = 10.0f32.powf(self.gain_db / 40.0);
                let two_sqrt_a_alpha = 2.0 * a_gain.sqrt() * alpha;
                let b0 = a_gain * ((a_gain + 1.0) - (a_gain - 1.0) * cos_w0 + two_sqrt_a_alpha);
                let b1 = 2.0 * a_gain * ((a_gain - 1.0) - (a_gain + 1.0) * cos_w0);
                let b2 = a_gain * ((a_gain + 1.0) - (a_gain - 1.0) * cos_w0 - two_sqrt_a_alpha);
                let a0 = (a_gain + 1.0) + (a_gain - 1.0) * cos_w0 + two_sqrt_a_alpha;
                let a1 = -2.0 * ((a_gain - 1.0) + (a_gain + 1.0) * cos_w0);
                let a2 = (a_gain + 1.0) + (a_gain - 1.0) * cos_w0 - two_sqrt_a_alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::HighShelf => {
                let a_gain = 10.0f32.powf(self.gain_db / 40.0);
                let two_sqrt_a_alpha = 2.0 * a_gain.sqrt() * alpha;
                let b0 = a_gain * ((a_gain + 1.0) + (a_gain - 1.0) * cos_w0 + two_sqrt_a_alpha);
                let b1 = -2.0 * a_gain * ((a_gain - 1.0) + (a_gain + 1.0) * cos_w0);
                let b2 = a_gain * ((a_gain + 1.0) + (a_gain - 1.0) * cos_w0 - two_sqrt_a_alpha);
                let a0 = (a_gain + 1.0) - (a_gain - 1.0) * cos_w0 + two_sqrt_a_alpha;
                let a1 = 2.0 * ((a_gain - 1.0) - (a_gain + 1.0) * cos_w0);
                let a2 = (a_gain + 1.0) - (a_gain - 1.0) * cos_w0 - two_sqrt_a_alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::Peak => {
                let a_gain = 10.0f32.powf(self.gain_db / 40.0);
                let b0 = 1.0 + alpha * a_gain;
                let b1 = -2.0 * cos_w0;
                let b2 = 1.0 - alpha * a_gain;
                let a0 = 1.0 + alpha / a_gain;
                let a1 = -2.0 * cos_w0;
                let a2 = 1.0 - alpha / a_gain;
                (b0, b1, b2, a0, a1, a2)
            }
        };

        // Normalize by a0
        let inv_a0 = 1.0 / a0;
        self.b0 = b0 * inv_a0;
        self.b1 = b1 * inv_a0;
        self.b2 = b2 * inv_a0;
        self.a1 = a1 * inv_a0;
        self.a2 = a2 * inv_a0;
    }

    /// Process a single sample through the filter.
    ///
    /// Uses Direct Form II Transposed for numerical stability.
    /// State variables are flushed to prevent denormal slowdowns.
    #[inline]
    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let output = self.b0 * input + self.z1;
        self.z1 = crate::flush_denormal(self.b1 * input - self.a1 * output + self.z2);
        self.z2 = crate::flush_denormal(self.b2 * input - self.a2 * output);
        output
    }

    /// Update filter parameters and recalculate coefficients.
    ///
    /// # Errors
    ///
    /// Returns error if frequency or q is invalid.
    pub fn set_params(&mut self, frequency: f32, q: f32, gain_db: f32) -> Result<()> {
        if let Some(e) = error::validate_frequency(frequency, self.sample_rate) {
            return Err(e);
        }
        if q <= 0.0 || !q.is_finite() {
            return Err(NaadError::InvalidParameter {
                name: "q".to_string(),
                reason: "must be > 0".to_string(),
            });
        }
        self.frequency = frequency;
        self.q = q;
        self.gain_db = gain_db;
        self.compute_coefficients();
        Ok(())
    }

    /// Process a buffer of samples in place.
    #[inline]
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process_sample(*sample);
        }
    }

    /// Reset filter state (clear delay line).
    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

/// Output from a state variable filter (simultaneous outputs).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SvfOutput {
    /// Low-pass output.
    pub low_pass: f32,
    /// High-pass output.
    pub high_pass: f32,
    /// Band-pass output.
    pub band_pass: f32,
    /// Notch output.
    pub notch: f32,
}

/// State variable filter with simultaneous LP/HP/BP/Notch outputs.
///
/// Uses the Cytomic/Simper SVF topology for numerical stability at high
/// resonance and high frequencies. Coefficients are cached and only
/// recomputed when parameters change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateVariableFilter {
    frequency: f32,
    q: f32,
    sample_rate: f32,
    // Cached coefficients
    g: f32,
    k: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    // Internal state
    #[serde(skip)]
    ic1eq: f32,
    #[serde(skip)]
    ic2eq: f32,
}

impl StateVariableFilter {
    /// Create a new state variable filter.
    ///
    /// # Errors
    ///
    /// Returns error if frequency, sample_rate, or q is invalid.
    pub fn new(frequency: f32, q: f32, sample_rate: f32) -> Result<Self> {
        if let Some(e) = error::validate_sample_rate(sample_rate) {
            return Err(e);
        }
        if let Some(e) = error::validate_frequency(frequency, sample_rate) {
            return Err(e);
        }
        if q <= 0.0 || !q.is_finite() {
            return Err(NaadError::InvalidParameter {
                name: "q".to_string(),
                reason: "must be > 0".to_string(),
            });
        }

        let g = (std::f32::consts::PI * frequency / sample_rate).tan();
        let k = 1.0 / q;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;

        Ok(Self {
            frequency,
            q,
            sample_rate,
            g,
            k,
            a1,
            a2,
            a3,
            ic1eq: 0.0,
            ic2eq: 0.0,
        })
    }

    /// Returns the current cutoff frequency in Hz.
    #[inline]
    #[must_use]
    pub fn frequency(&self) -> f32 {
        self.frequency
    }

    /// Returns the current Q factor.
    #[inline]
    #[must_use]
    pub fn q(&self) -> f32 {
        self.q
    }

    /// Returns the sample rate in Hz.
    #[inline]
    #[must_use]
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Update filter parameters. Coefficients are recalculated only when called.
    ///
    /// # Errors
    ///
    /// Returns error if frequency or q is invalid.
    pub fn set_params(&mut self, frequency: f32, q: f32) -> Result<()> {
        if let Some(e) = error::validate_frequency(frequency, self.sample_rate) {
            return Err(e);
        }
        if q <= 0.0 || !q.is_finite() {
            return Err(NaadError::InvalidParameter {
                name: "q".to_string(),
                reason: "must be > 0".to_string(),
            });
        }

        self.frequency = frequency;
        self.q = q;
        self.g = (std::f32::consts::PI * frequency / self.sample_rate).tan();
        self.k = 1.0 / q;
        self.a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
        self.a2 = self.g * self.a1;
        self.a3 = self.g * self.a2;
        Ok(())
    }

    /// Process a sample and return all four filter outputs simultaneously.
    ///
    /// State variables are flushed to prevent denormal slowdowns.
    #[inline]
    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> SvfOutput {
        let v3 = input - self.ic2eq;
        let v1 = self.a1 * self.ic1eq + self.a2 * v3;
        let v2 = self.ic2eq + self.a2 * self.ic1eq + self.a3 * v3;

        self.ic1eq = crate::flush_denormal(2.0 * v1 - self.ic1eq);
        self.ic2eq = crate::flush_denormal(2.0 * v2 - self.ic2eq);

        let low_pass = v2;
        let band_pass = v1;
        let high_pass = input - self.k * v1 - v2;
        let notch = low_pass + high_pass;

        SvfOutput {
            low_pass,
            high_pass,
            band_pass,
            notch,
        }
    }

    /// Process a single sample returning only the low-pass output.
    ///
    /// Convenience method — internally computes all outputs.
    #[inline]
    #[must_use]
    pub fn process_sample_lowpass(&mut self, input: f32) -> f32 {
        self.process_sample(input).low_pass
    }

    /// Process a buffer of samples, writing low-pass output in-place.
    #[inline]
    pub fn process_buffer_lowpass(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process_sample(*sample).low_pass;
        }
    }

    /// Reset filter state.
    pub fn reset(&mut self) {
        self.ic1eq = 0.0;
        self.ic2eq = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lowpass_basic() {
        let mut filter = BiquadFilter::new(FilterType::LowPass, 44100.0, 1000.0, 0.707).unwrap();
        // DC should pass through
        let mut output = 0.0;
        for _ in 0..1000 {
            output = filter.process_sample(1.0);
        }
        assert!(
            (output - 1.0).abs() < 0.01,
            "DC should pass through LP, got {output}"
        );
    }

    #[test]
    fn test_highpass_blocks_dc() {
        let mut filter = BiquadFilter::new(FilterType::HighPass, 44100.0, 1000.0, 0.707).unwrap();
        let mut output = 0.0;
        for _ in 0..1000 {
            output = filter.process_sample(1.0);
        }
        assert!(output.abs() < 0.01, "HP should block DC, got {output}");
    }

    #[test]
    fn test_invalid_params() {
        assert!(BiquadFilter::new(FilterType::LowPass, 0.0, 1000.0, 0.7).is_err());
        assert!(BiquadFilter::new(FilterType::LowPass, 44100.0, 0.0, 0.7).is_err());
        assert!(BiquadFilter::new(FilterType::LowPass, 44100.0, 1000.0, 0.0).is_err());
        assert!(BiquadFilter::new(FilterType::LowPass, 44100.0, 30000.0, 0.7).is_err());
    }

    #[test]
    fn test_set_params() {
        let mut filter = BiquadFilter::new(FilterType::LowPass, 44100.0, 1000.0, 0.707).unwrap();
        assert!(filter.set_params(2000.0, 1.0, 0.0).is_ok());
        assert!(filter.set_params(0.0, 1.0, 0.0).is_err());
    }

    #[test]
    fn test_svf_basic() {
        let mut svf = StateVariableFilter::new(1000.0, 0.707, 44100.0).unwrap();
        let out = svf.process_sample(1.0);
        assert!(out.low_pass.is_finite());
        assert!(out.high_pass.is_finite());
        assert!(out.band_pass.is_finite());
        assert!(out.notch.is_finite());
    }

    #[test]
    fn test_serde_roundtrip() {
        let filter = BiquadFilter::new(FilterType::BandPass, 44100.0, 1000.0, 2.0).unwrap();
        let json = serde_json::to_string(&filter).unwrap();
        let back: BiquadFilter = serde_json::from_str(&json).unwrap();
        assert!((filter.frequency - back.frequency).abs() < f32::EPSILON);
    }

    #[test]
    fn test_process_buffer() {
        let mut filter = BiquadFilter::new(FilterType::LowPass, 44100.0, 1000.0, 0.707).unwrap();
        let mut buf = [1.0f32; 256];
        filter.process_buffer(&mut buf);
        assert!(buf.iter().all(|s| s.is_finite()));
    }
}
