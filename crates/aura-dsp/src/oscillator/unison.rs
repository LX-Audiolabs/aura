//! Unison oscillator — N detuned voices mixed together.

use serde::{Deserialize, Serialize};

use super::core::{Waveform, stateless_waveform_sample};
use crate::error::{self, Result};

fn default_detune_ratios() -> [f32; 8] {
    [1.0; 8]
}

fn default_ratios_dirty() -> bool {
    true
}

/// Unison oscillator — N detuned copies of a waveform mixed together.
///
/// Produces a thicker sound by layering multiple slightly-detuned voices.
/// Voices are spread symmetrically around the center frequency. Supports
/// stereo width output via [`Self::next_sample_stereo`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnisonOscillator {
    /// Waveform for all voices.
    waveform: Waveform,
    /// Base frequency in Hz.
    frequency: f32,
    /// Sample rate in Hz.
    sample_rate: f32,
    /// Number of unison voices (1-8).
    num_voices: usize,
    /// Detune amount in cents (spread across all voices).
    detune_cents: f32,
    /// Stereo spread (0.0 = mono, 1.0 = full width).
    stereo_spread: f32,
    /// Per-voice phases.
    phases: [f32; 8],
    /// Precomputed detune ratios.
    #[serde(default = "default_detune_ratios")]
    detune_ratios: [f32; 8],
    /// Whether ratios need recomputing.
    #[serde(default = "default_ratios_dirty")]
    ratios_dirty: bool,
}

impl UnisonOscillator {
    /// Create a new unison oscillator.
    ///
    /// # Arguments
    ///
    /// * `waveform` - Waveform type for all voices
    /// * `frequency` - Base frequency in Hz
    /// * `num_voices` - Number of unison voices (clamped to 1..8)
    /// * `detune_cents` - Total detune spread in cents
    /// * `sample_rate` - Sample rate in Hz
    ///
    /// # Errors
    ///
    /// Returns error if frequency or sample_rate is invalid.
    pub fn new(
        waveform: Waveform,
        frequency: f32,
        num_voices: usize,
        detune_cents: f32,
        sample_rate: f32,
    ) -> Result<Self> {
        if let Some(e) = error::validate_sample_rate(sample_rate) {
            return Err(e);
        }
        if let Some(e) = error::validate_frequency(frequency, sample_rate) {
            return Err(e);
        }

        let nv = num_voices.clamp(1, 8);

        // Randomize initial phases for a natural sound.
        let mut phases = [0.0f32; 8];
        let mut seed = 12345u32;
        for p in phases.iter_mut().take(nv) {
            *p = crate::dsp_util::xorshift32_unit_f32(&mut seed);
        }

        let mut osc = Self {
            waveform,
            frequency,
            sample_rate,
            num_voices: nv,
            detune_cents: detune_cents.max(0.0),
            stereo_spread: 0.5,
            phases,
            detune_ratios: [1.0; 8],
            ratios_dirty: true,
        };
        osc.recompute_ratios();
        Ok(osc)
    }

    /// Recompute detune ratios from cents spread.
    fn recompute_ratios(&mut self) {
        if self.num_voices <= 1 {
            self.detune_ratios[0] = 1.0;
        } else {
            for i in 0..self.num_voices {
                // Spread voices symmetrically: -detune_cents/2 to +detune_cents/2
                let t = i as f32 / (self.num_voices - 1) as f32; // 0..1
                let cents_offset = (t - 0.5) * self.detune_cents;
                // Convert cents to frequency ratio: 2^(cents/1200)
                self.detune_ratios[i] = (cents_offset / 1200.0).exp2();
            }
        }
        self.ratios_dirty = false;
    }

    /// Generate the next mono sample (all voices averaged).
    #[inline]
    #[must_use]
    pub fn next_sample(&mut self) -> f32 {
        if self.ratios_dirty {
            self.recompute_ratios();
        }

        let mut sum = 0.0f32;
        let nv = self.num_voices;

        for i in 0..nv {
            let freq = self.frequency * self.detune_ratios[i];
            let dt = freq / self.sample_rate;
            let t = self.phases[i];

            let sample = stateless_waveform_sample(self.waveform, t, dt);

            sum += sample;

            // Advance phase
            self.phases[i] += dt;
            if self.phases[i] >= 1.0 {
                self.phases[i] -= 1.0;
            }
        }

        sum / nv as f32
    }

    /// Generate the next stereo sample pair (left, right).
    ///
    /// Voices are panned across the stereo field based on `stereo_spread`.
    #[inline]
    #[must_use]
    pub fn next_sample_stereo(&mut self) -> (f32, f32) {
        if self.ratios_dirty {
            self.recompute_ratios();
        }

        let mut left = 0.0f32;
        let mut right = 0.0f32;
        let nv = self.num_voices;

        for i in 0..nv {
            let freq = self.frequency * self.detune_ratios[i];
            let dt = freq / self.sample_rate;
            let t = self.phases[i];

            let sample = stateless_waveform_sample(self.waveform, t, dt);

            // Pan position: voice 0 = left, voice N-1 = right
            if nv > 1 {
                let pan = i as f32 / (nv - 1) as f32; // 0..1
                let w = self.stereo_spread * 0.5;
                let pan_scaled = 0.5 + (pan - 0.5) * w * 2.0;
                // Equal-power panning
                let angle = pan_scaled * std::f32::consts::FRAC_PI_2;
                left += sample * angle.cos();
                right += sample * angle.sin();
            } else {
                left += sample * std::f32::consts::FRAC_1_SQRT_2;
                right += sample * std::f32::consts::FRAC_1_SQRT_2;
            }

            self.phases[i] += dt;
            if self.phases[i] >= 1.0 {
                self.phases[i] -= 1.0;
            }
        }

        let inv = 1.0 / nv as f32;
        (left * inv, right * inv)
    }

    /// Fill a mono buffer.
    #[inline]
    pub fn fill_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.next_sample();
        }
    }

    /// Fill stereo buffers (left and right channels).
    #[inline]
    pub fn fill_buffer_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let (sl, sr) = self.next_sample_stereo();
            *l = sl;
            *r = sr;
        }
    }

    /// Set the number of unison voices (1-8).
    pub fn set_num_voices(&mut self, n: usize) {
        self.num_voices = n.clamp(1, 8);
        self.ratios_dirty = true;
    }

    /// Set the detune amount in cents.
    pub fn set_detune_cents(&mut self, cents: f32) {
        self.detune_cents = cents.max(0.0);
        self.ratios_dirty = true;
    }

    /// Set the stereo spread (0.0 = mono, 1.0 = full width).
    pub fn set_stereo_spread(&mut self, spread: f32) {
        self.stereo_spread = spread.clamp(0.0, 1.0);
    }

    /// Set the base frequency.
    ///
    /// # Errors
    ///
    /// Returns error if frequency is invalid.
    pub fn set_frequency(&mut self, freq: f32) -> Result<()> {
        if let Some(e) = error::validate_frequency(freq, self.sample_rate) {
            return Err(e);
        }
        self.frequency = freq;
        Ok(())
    }

    /// Returns the number of unison voices.
    #[inline]
    #[must_use]
    pub fn num_voices(&self) -> usize {
        self.num_voices
    }

    /// Returns the base frequency.
    #[inline]
    #[must_use]
    pub fn frequency(&self) -> f32 {
        self.frequency
    }

    /// Returns the detune in cents.
    #[inline]
    #[must_use]
    pub fn detune_cents(&self) -> f32 {
        self.detune_cents
    }
}

#[cfg(test)]
mod tests {
    use super::super::core::Oscillator;
    use super::*;

    #[test]
    fn test_unison_mono() {
        let mut uni = UnisonOscillator::new(Waveform::Saw, 440.0, 4, 10.0, 44100.0).unwrap();
        let mut buf = [0.0f32; 1024];
        uni.fill_buffer(&mut buf);
        assert!(buf.iter().all(|s| s.is_finite()));
        assert!(buf.iter().any(|&s| s != 0.0));
    }

    #[test]
    fn test_unison_stereo_spread() {
        let mut uni = UnisonOscillator::new(Waveform::Saw, 440.0, 4, 10.0, 44100.0).unwrap();
        uni.set_stereo_spread(1.0);
        let mut left = [0.0f32; 512];
        let mut right = [0.0f32; 512];
        uni.fill_buffer_stereo(&mut left, &mut right);
        // With spread, left and right should differ
        let diff: f32 = left
            .iter()
            .zip(right.iter())
            .map(|(l, r)| (l - r).abs())
            .sum();
        assert!(diff > 0.01, "stereo channels should differ with spread=1.0");
    }

    #[test]
    fn test_unison_single_voice() {
        let mut uni = UnisonOscillator::new(Waveform::Sine, 440.0, 1, 0.0, 44100.0).unwrap();
        let mut osc = Oscillator::new(Waveform::Sine, 440.0, 44100.0).unwrap();
        // Single voice unison with 0 detune should match a plain oscillator
        // (phase randomization makes exact match impossible, but range should match)
        for _ in 0..100 {
            let s = uni.next_sample();
            assert!((-1.01..=1.01).contains(&s));
            let _ = osc.next_sample();
        }
    }

    #[test]
    fn test_unison_serde_roundtrip() {
        let uni = UnisonOscillator::new(Waveform::Saw, 440.0, 4, 15.0, 44100.0).unwrap();
        let json = serde_json::to_string(&uni).unwrap();
        let back: UnisonOscillator = serde_json::from_str(&json).unwrap();
        assert_eq!(uni.num_voices(), back.num_voices());
        assert!((uni.detune_cents() - back.detune_cents()).abs() < f32::EPSILON);
    }
}
