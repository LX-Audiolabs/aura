//! Parametric and graphic equalizers.
//!
//! Wraps [`BiquadFilter`] into multi-band EQ
//! configurations for spectral shaping.

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::error::Result;
use crate::filter::{BiquadFilter, FilterType};

/// Inline band capacity for parametric/graphic EQ — typical EQs run
/// 4–10 parametric bands or up to 10 graphic bands; 16 inline covers
/// all common cases without spilling to the heap.
const EQ_INLINE: usize = 16;

/// One band of a [`ParametricEq`].
///
/// Wraps a [`BiquadFilter`] (configured at band-add time with type, frequency,
/// Q, and gain) plus an `enabled` toggle so bands can be bypassed without
/// removing them. Disabled bands are skipped during processing — their state
/// is preserved but contributes no filtering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqBand {
    /// The underlying biquad filter.
    filter: BiquadFilter,
    /// Whether this band is enabled. When `false`, the band is bypassed.
    pub enabled: bool,
}

/// N-band parametric equalizer.
///
/// Each band is an independent [`BiquadFilter`] with configurable type,
/// frequency, Q, and gain. Bands are processed in series. Storage is a
/// [`SmallVec`] with `EQ_INLINE = 16` slots inline — typical EQ
/// configurations stay stack-resident.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParametricEq {
    /// The EQ bands.
    bands: SmallVec<[EqBand; EQ_INLINE]>,
    /// Sample rate in Hz.
    sample_rate: f32,
}

impl ParametricEq {
    /// Create a new parametric EQ with no bands.
    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        Self {
            bands: SmallVec::new(),
            sample_rate,
        }
    }

    /// Add a band to the EQ.
    ///
    /// # Errors
    ///
    /// Returns error if filter parameters are invalid.
    pub fn add_band(
        &mut self,
        filter_type: FilterType,
        frequency: f32,
        q: f32,
        gain_db: f32,
    ) -> Result<()> {
        let filter = BiquadFilter::with_gain(filter_type, self.sample_rate, frequency, q, gain_db)?;
        self.bands.push(EqBand {
            filter,
            enabled: true,
        });
        Ok(())
    }

    /// Returns the number of bands.
    #[must_use]
    pub fn num_bands(&self) -> usize {
        self.bands.len()
    }

    /// Get a mutable reference to a band for parameter changes.
    pub fn band_mut(&mut self, index: usize) -> Option<&mut EqBand> {
        self.bands.get_mut(index)
    }

    /// Process a single sample through all enabled bands in series.
    #[inline]
    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let mut out = input;
        for band in &mut self.bands {
            if band.enabled {
                out = band.filter.process_sample(out);
            }
        }
        out
    }

    /// Process a buffer in place.
    #[inline]
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for s in buffer.iter_mut() {
            *s = self.process_sample(*s);
        }
    }

    /// Reset all band filter states.
    pub fn reset(&mut self) {
        for band in &mut self.bands {
            band.filter.reset();
        }
    }
}

/// 10-band graphic equalizer with ISO center frequencies.
///
/// Bands at: 31, 63, 125, 250, 500, 1k, 2k, 4k, 8k, 16k Hz.
/// Each band is a peak filter with configurable gain.
/// Bands above Nyquist are automatically excluded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphicEq {
    /// Internal parametric EQ.
    eq: ParametricEq,
    /// Frequencies of the active bands (may be fewer than 10 at low sample rates).
    active_frequencies: Vec<f32>,
}

/// ISO center frequencies for the 10-band graphic EQ.
pub const GRAPHIC_EQ_FREQUENCIES: [f32; 10] = [
    31.25, 62.5, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];

impl GraphicEq {
    /// Create a new 10-band graphic EQ with all bands at 0 dB.
    ///
    /// # Errors
    ///
    /// Returns error if sample_rate is invalid.
    pub fn new(sample_rate: f32) -> Result<Self> {
        let mut eq = ParametricEq::new(sample_rate);
        let mut active_frequencies = Vec::new();
        let q = 1.4; // ~1 octave bandwidth
        for &freq in &GRAPHIC_EQ_FREQUENCIES {
            // Skip bands above Nyquist
            if freq < sample_rate * 0.5 {
                eq.add_band(FilterType::Peak, freq, q, 0.0)?;
                active_frequencies.push(freq);
            }
        }
        Ok(Self {
            eq,
            active_frequencies,
        })
    }

    /// Set the gain for a band (0-9) in dB.
    ///
    /// # Errors
    ///
    /// Returns error if index is out of range or gain is invalid.
    pub fn set_band_gain(&mut self, index: usize, gain_db: f32) -> Result<()> {
        if index >= self.active_frequencies.len() {
            return Err(crate::NaadError::InvalidParameter {
                name: "index".to_string(),
                reason: format!(
                    "band index {index} out of range ({}  active bands)",
                    self.active_frequencies.len()
                ),
            });
        }
        let freq = self.active_frequencies[index];
        if let Some(band) = self.eq.band_mut(index) {
            band.filter.set_params(freq, 1.4, gain_db)
        } else {
            Err(crate::NaadError::InvalidParameter {
                name: "index".to_string(),
                reason: format!("band index {index} out of range"),
            })
        }
    }

    /// Process a single sample.
    #[inline]
    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        self.eq.process_sample(input)
    }

    /// Process a buffer in place.
    #[inline]
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        self.eq.process_buffer(buffer);
    }

    /// Returns the number of active bands.
    #[must_use]
    pub fn num_bands(&self) -> usize {
        self.eq.num_bands()
    }
}

/// De-esser — reduces sibilance in the 4-8 kHz range.
///
/// Uses a bandpass sidechain to detect sibilant energy, then applies
/// gain reduction to the full signal when sibilance exceeds the threshold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeEsser {
    /// Detection bandpass filter (sidechain).
    detector_filter: BiquadFilter,
    /// Envelope follower for sidechain.
    envelope: f32,
    /// Threshold in dB.
    pub threshold_db: f32,
    /// Maximum reduction in dB.
    pub max_reduction_db: f32,
    /// Envelope smoothing coefficient.
    smooth_coeff: f32,
}

impl DeEsser {
    /// Create a new de-esser.
    ///
    /// * `center_freq` - Center frequency for sibilance detection (typically 6000-8000 Hz)
    /// * `threshold_db` - Threshold above which reduction is applied
    /// * `sample_rate` - Sample rate in Hz
    ///
    /// # Errors
    ///
    /// Returns error if parameters are invalid.
    pub fn new(center_freq: f32, threshold_db: f32, sample_rate: f32) -> Result<Self> {
        let detector_filter =
            BiquadFilter::new(FilterType::BandPass, sample_rate, center_freq, 2.0)?;
        let smooth_coeff = 1.0 - (-1.0 / (0.002 * sample_rate)).exp(); // ~2ms

        Ok(Self {
            detector_filter,
            envelope: 0.0,
            threshold_db,
            max_reduction_db: -12.0,
            smooth_coeff,
        })
    }

    /// Process a single sample.
    #[inline]
    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        // Sidechain: bandpass the input to detect sibilance
        let detected = self.detector_filter.process_sample(input);
        let level = detected.abs();

        // Envelope follower
        self.envelope += self.smooth_coeff * (level - self.envelope);
        self.envelope = crate::flush_denormal(self.envelope);

        let env_db = crate::dsp_util::amplitude_to_db(self.envelope);

        if env_db > self.threshold_db {
            let overshoot = env_db - self.threshold_db;
            let reduction_db = (-overshoot).max(self.max_reduction_db);
            input * crate::dsp_util::db_to_amplitude(reduction_db)
        } else {
            input
        }
    }

    /// Process a buffer in place.
    #[inline]
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for s in buffer.iter_mut() {
            *s = self.process_sample(*s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parametric_eq_passthrough() {
        let mut eq = ParametricEq::new(44100.0);
        // No bands = passthrough
        let out = eq.process_sample(0.5);
        assert!((out - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parametric_eq_with_band() {
        let mut eq = ParametricEq::new(44100.0);
        eq.add_band(FilterType::Peak, 1000.0, 1.0, 6.0).unwrap();
        assert_eq!(eq.num_bands(), 1);
        let out = eq.process_sample(0.5);
        assert!(out.is_finite());
    }

    #[test]
    fn test_graphic_eq() {
        let mut geq = GraphicEq::new(44100.0).unwrap();
        assert_eq!(geq.num_bands(), 10);
        // Set a boost
        geq.set_band_gain(4, 6.0).unwrap(); // 500 Hz +6dB
        let out = geq.process_sample(0.5);
        assert!(out.is_finite());
    }

    #[test]
    fn test_graphic_eq_invalid_band() {
        let mut geq = GraphicEq::new(44100.0).unwrap();
        assert!(geq.set_band_gain(20, 6.0).is_err());
    }

    #[test]
    fn test_deesser() {
        let mut de = DeEsser::new(6000.0, -20.0, 44100.0).unwrap();
        let out = de.process_sample(0.5);
        assert!(out.is_finite());
    }

    #[test]
    fn test_serde_roundtrip_parametric_eq() {
        let mut eq = ParametricEq::new(44100.0);
        eq.add_band(FilterType::Peak, 1000.0, 1.0, 3.0).unwrap();
        let json = serde_json::to_string(&eq).unwrap();
        let back: ParametricEq = serde_json::from_str(&json).unwrap();
        assert_eq!(eq.num_bands(), back.num_bands());
    }

    #[test]
    fn test_serde_roundtrip_graphic_eq() {
        let geq = GraphicEq::new(44100.0).unwrap();
        let json = serde_json::to_string(&geq).unwrap();
        let _back: GraphicEq = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_serde_roundtrip_deesser() {
        let de = DeEsser::new(6000.0, -20.0, 44100.0).unwrap();
        let json = serde_json::to_string(&de).unwrap();
        let back: DeEsser = serde_json::from_str(&json).unwrap();
        assert!((de.threshold_db - back.threshold_db).abs() < f32::EPSILON);
    }
}
