//! Formant synthesis for vowel sounds.
//!
//! Models the human vocal tract as a parallel bank of bandpass resonators.
//! Each vowel is characterised by three formant frequencies (F1, F2, F3)
//! with associated bandwidths and amplitudes. Supports smooth morphing
//! between vowel shapes.

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::filter::{BiquadFilter, FilterType};

/// Number of formant bands.
const NUM_FORMANTS: usize = 3;

/// Formant parameter set: (frequency Hz, bandwidth Hz, amplitude linear).
type FormantParams = [(f32, f32, f32); NUM_FORMANTS];

/// Vowel type for formant synthesis.
///
/// Standard IPA formant values for an adult male voice approximation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Vowel {
    /// /a/ as in "father".
    A,
    /// /e/ as in "hey".
    E,
    /// /i/ as in "see".
    I,
    /// /o/ as in "go".
    O,
    /// /u/ as in "boot".
    U,
}

impl Vowel {
    /// Returns the first three formant parameters: (freq, bandwidth, amplitude).
    ///
    /// Values are standard approximations for an adult male voice.
    #[must_use]
    pub fn formants(self) -> FormantParams {
        match self {
            Vowel::A => [
                (730.0, 90.0, 1.0),
                (1090.0, 110.0, 0.5),
                (2440.0, 170.0, 0.25),
            ],
            Vowel::E => [
                (530.0, 60.0, 1.0),
                (1840.0, 100.0, 0.4),
                (2480.0, 120.0, 0.2),
            ],
            Vowel::I => [
                (270.0, 60.0, 1.0),
                (2290.0, 90.0, 0.3),
                (3010.0, 100.0, 0.15),
            ],
            Vowel::O => [
                (570.0, 80.0, 1.0),
                (840.0, 100.0, 0.5),
                (2410.0, 120.0, 0.2),
            ],
            Vowel::U => [
                (300.0, 70.0, 1.0),
                (870.0, 100.0, 0.35),
                (2240.0, 120.0, 0.15),
            ],
        }
    }
}

/// Formant filter: a parallel bank of bandpass resonators.
///
/// Holds exactly 3 bands (F1/F2/F3), matching the standard vowel formant
/// model used elsewhere in the module. Storage is a stack-allocated array —
/// no heap indirection per sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormantFilter {
    /// Bandpass filter per formant.
    filters: [BiquadFilter; NUM_FORMANTS],
    /// Amplitude per formant.
    amplitudes: [f32; NUM_FORMANTS],
}

impl FormantFilter {
    /// Create a formant filter from explicit parameters.
    ///
    /// # Errors
    ///
    /// Returns [`crate::NaadError::InvalidParameter`] if `formants.len()` is
    /// not exactly 3, or if any filter parameters are invalid.
    pub fn new(formants: &[(f32, f32, f32)], sample_rate: f32) -> Result<Self> {
        if formants.len() != NUM_FORMANTS {
            return Err(crate::error::NaadError::InvalidParameter {
                name: "formants".to_string(),
                reason: format!(
                    "expected exactly {NUM_FORMANTS} formant bands, got {}",
                    formants.len()
                ),
            });
        }

        let q = |freq: f32, bw: f32| if bw > 0.0 { freq / bw } else { 1.0 };
        let filters: [BiquadFilter; NUM_FORMANTS] = [
            BiquadFilter::new(
                FilterType::BandPass,
                sample_rate,
                formants[0].0,
                q(formants[0].0, formants[0].1),
            )?,
            BiquadFilter::new(
                FilterType::BandPass,
                sample_rate,
                formants[1].0,
                q(formants[1].0, formants[1].1),
            )?,
            BiquadFilter::new(
                FilterType::BandPass,
                sample_rate,
                formants[2].0,
                q(formants[2].0, formants[2].1),
            )?,
        ];
        let amplitudes = [formants[0].2, formants[1].2, formants[2].2];

        Ok(Self {
            filters,
            amplitudes,
        })
    }

    /// Process one input sample through all parallel formant bands.
    #[inline]
    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let mut sum = 0.0f32;
        for (filter, &amp) in self.filters.iter_mut().zip(self.amplitudes.iter()) {
            sum += filter.process_sample(input) * amp;
        }
        sum
    }
}

/// Formant synthesis engine with vowel morphing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormantSynth {
    /// Per-formant bandpass filters.
    filters: [BiquadFilter; NUM_FORMANTS],
    /// Current formant amplitudes.
    amplitudes: [f32; NUM_FORMANTS],
    /// Current target formant parameters.
    target_params: FormantParams,
    /// Current vowel.
    current_vowel: Vowel,
    /// Sample rate in Hz.
    sample_rate: f32,
}

impl FormantSynth {
    /// Create a new formant synth initialised to the given vowel.
    ///
    /// # Errors
    ///
    /// Returns error if filter parameters are invalid.
    pub fn new(vowel: Vowel, sample_rate: f32) -> Result<Self> {
        let params = vowel.formants();
        let mut filters: [BiquadFilter; NUM_FORMANTS] = [
            BiquadFilter::new(
                FilterType::BandPass,
                sample_rate,
                params[0].0,
                params[0].0 / params[0].1,
            )?,
            BiquadFilter::new(
                FilterType::BandPass,
                sample_rate,
                params[1].0,
                params[1].0 / params[1].1,
            )?,
            BiquadFilter::new(
                FilterType::BandPass,
                sample_rate,
                params[2].0,
                params[2].0 / params[2].1,
            )?,
        ];
        let _ = &mut filters; // suppress unused_mut if needed

        let amplitudes = [params[0].2, params[1].2, params[2].2];

        Ok(Self {
            filters,
            amplitudes,
            target_params: params,
            current_vowel: vowel,
            sample_rate,
        })
    }

    /// Set the target vowel, updating filter parameters.
    ///
    /// # Errors
    ///
    /// Returns error if the resulting filter parameters are invalid.
    pub fn set_vowel(&mut self, vowel: Vowel) -> Result<()> {
        self.current_vowel = vowel;
        let params = vowel.formants();
        self.apply_params(&params)
    }

    /// Morph between two vowels at position `t` (0.0 = vowel_a, 1.0 = vowel_b).
    ///
    /// # Errors
    ///
    /// Returns error if the interpolated filter parameters are invalid.
    pub fn morph(&mut self, vowel_a: Vowel, vowel_b: Vowel, t: f32) -> Result<()> {
        let t = t.clamp(0.0, 1.0);
        let a = vowel_a.formants();
        let b = vowel_b.formants();

        let mut params: FormantParams = [(0.0, 0.0, 0.0); NUM_FORMANTS];
        for i in 0..NUM_FORMANTS {
            params[i].0 = a[i].0 + (b[i].0 - a[i].0) * t;
            params[i].1 = a[i].1 + (b[i].1 - a[i].1) * t;
            params[i].2 = a[i].2 + (b[i].2 - a[i].2) * t;
        }

        self.apply_params(&params)
    }

    /// Process one input sample through the parallel formant bank.
    #[inline]
    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let mut sum = 0.0f32;
        for i in 0..NUM_FORMANTS {
            sum += self.filters[i].process_sample(input) * self.amplitudes[i];
        }
        sum
    }

    /// Fill a buffer by processing each input sample in place.
    #[inline]
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for s in buffer.iter_mut() {
            *s = self.process_sample(*s);
        }
    }

    /// Returns the current vowel.
    #[must_use]
    pub fn current_vowel(&self) -> Vowel {
        self.current_vowel
    }

    /// Apply formant parameters to the filters.
    fn apply_params(&mut self, params: &FormantParams) -> Result<()> {
        self.target_params = *params;
        for (i, &(freq, bw, amp)) in params.iter().enumerate().take(NUM_FORMANTS) {
            let q = if bw > 0.0 { freq / bw } else { 1.0 };
            self.filters[i].set_params(freq, q, 0.0)?;
            self.amplitudes[i] = amp;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_different_vowels_produce_different_output() {
        // Feed the same input to two different vowel configs.
        let mut synth_a = FormantSynth::new(Vowel::A, 44100.0).unwrap();
        let mut synth_i = FormantSynth::new(Vowel::I, 44100.0).unwrap();

        // Use a simple impulse train as excitation.
        let mut out_a = Vec::with_capacity(512);
        let mut out_i = Vec::with_capacity(512);
        for n in 0..512 {
            let input = if n % 100 == 0 { 1.0 } else { 0.0 };
            out_a.push(synth_a.process_sample(input));
            out_i.push(synth_i.process_sample(input));
        }

        let diff: f32 = out_a
            .iter()
            .zip(out_i.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(
            diff > 0.01,
            "different vowels should produce different spectra"
        );
    }

    #[test]
    fn test_morph() {
        let mut synth = FormantSynth::new(Vowel::A, 44100.0).unwrap();
        synth.morph(Vowel::A, Vowel::O, 0.5).unwrap();
        // Just ensure it doesn't panic and produces output.
        let input = 1.0;
        let out = synth.process_sample(input);
        assert!(out.is_finite());
    }

    #[test]
    fn test_formant_filter_standalone() {
        let params = Vowel::E.formants();
        let mut ff = FormantFilter::new(&params, 44100.0).unwrap();
        let out = ff.process_sample(1.0);
        assert!(out.is_finite());
    }

    #[test]
    fn test_serde_roundtrip() {
        let synth = FormantSynth::new(Vowel::A, 44100.0).unwrap();
        let json = serde_json::to_string(&synth).unwrap();
        let back: FormantSynth = serde_json::from_str(&json).unwrap();
        assert_eq!(synth.current_vowel, back.current_vowel);
    }

    #[test]
    fn test_vowel_formants_are_valid() {
        for vowel in &[Vowel::A, Vowel::E, Vowel::I, Vowel::O, Vowel::U] {
            let params = vowel.formants();
            for (freq, bw, amp) in &params {
                assert!(*freq > 0.0);
                assert!(*bw > 0.0);
                assert!(*amp > 0.0);
            }
        }
    }
}
