//! Audio effects: chorus, flanger, phaser, and distortion.

use serde::{Deserialize, Serialize};

use crate::delay::DelayLine;
use crate::error::{self, Result};
use crate::oscillator::{Oscillator, Waveform};

/// Distortion type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DistortionType {
    /// Soft clipping using tanh saturation.
    SoftClip,
    /// Hard clipping (clamping).
    HardClip,
    /// Wave folding (triangle fold).
    WaveFold,
}

/// Chorus effect — multi-tap modulated delay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chorus {
    /// Delay lines for each voice.
    delay_lines: Vec<DelayLine>,
    /// LFO oscillators for each voice.
    lfos: Vec<Oscillator>,
    /// Base delay in samples.
    base_delay: f32,
    /// Depth of modulation in samples.
    depth: f32,
    /// Wet/dry mix (0.0 = dry, 1.0 = wet).
    pub mix: f32,
    /// Number of voices.
    num_voices: usize,
}

impl Chorus {
    /// Create a new chorus effect.
    ///
    /// # Arguments
    ///
    /// * `num_voices` - Number of chorus voices (typically 2-4)
    /// * `base_delay_ms` - Base delay in milliseconds (typically 7-20ms)
    /// * `depth_ms` - Modulation depth in milliseconds (typically 1-5ms)
    /// * `rate` - LFO rate in Hz (typically 0.1-5Hz)
    /// * `mix` - Wet/dry mix (0.0 to 1.0)
    /// * `sample_rate` - Sample rate in Hz
    ///
    /// # Errors
    ///
    /// Returns error if sample_rate or LFO rate is invalid.
    pub fn new(
        num_voices: usize,
        base_delay_ms: f32,
        depth_ms: f32,
        rate: f32,
        mix: f32,
        sample_rate: f32,
    ) -> Result<Self> {
        if let Some(e) = error::validate_sample_rate(sample_rate) {
            return Err(e);
        }

        let num_voices = num_voices.max(1);
        let base_delay = base_delay_ms * sample_rate / 1000.0;
        let depth = depth_ms * sample_rate / 1000.0;
        let max_delay = (base_delay + depth + 1.0) as usize;

        let mut delay_lines = Vec::with_capacity(num_voices);
        let mut lfos = Vec::with_capacity(num_voices);

        for i in 0..num_voices {
            delay_lines.push(DelayLine::new(max_delay));
            // Spread LFO phases across voices
            let mut lfo = Oscillator::new(Waveform::Sine, rate.max(0.01), sample_rate)?;
            lfo.set_phase(i as f32 / num_voices as f32);
            lfos.push(lfo);
        }

        Ok(Self {
            delay_lines,
            lfos,
            base_delay,
            depth,
            mix: mix.clamp(0.0, 1.0),
            num_voices,
        })
    }

    /// Process a single sample through the chorus.
    #[inline]
    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let mut wet = 0.0f32;

        for i in 0..self.num_voices {
            let lfo_val = self.lfos[i].next_sample();
            let delay = self.base_delay + self.depth * (lfo_val * 0.5 + 0.5);

            self.delay_lines[i].write(input);
            wet += self.delay_lines[i].read(delay);
        }

        wet /= self.num_voices as f32;
        input * (1.0 - self.mix) + wet * self.mix
    }
}

/// Flanger effect — short feedback delay with LFO modulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flanger {
    /// Delay line.
    delay_line: DelayLine,
    /// LFO oscillator.
    lfo: Oscillator,
    /// Base delay in samples.
    base_delay: f32,
    /// Modulation depth in samples.
    depth: f32,
    /// Feedback coefficient.
    pub feedback: f32,
    /// Wet/dry mix (0.0 to 1.0).
    pub mix: f32,
    /// Previous output for feedback.
    #[serde(skip)]
    prev_output: f32,
}

impl Flanger {
    /// Create a new flanger effect.
    ///
    /// # Errors
    ///
    /// Returns error if sample_rate or LFO rate is invalid.
    pub fn new(
        base_delay_ms: f32,
        depth_ms: f32,
        rate: f32,
        feedback: f32,
        mix: f32,
        sample_rate: f32,
    ) -> Result<Self> {
        if let Some(e) = error::validate_sample_rate(sample_rate) {
            return Err(e);
        }

        let base_delay = base_delay_ms * sample_rate / 1000.0;
        let depth = depth_ms * sample_rate / 1000.0;
        let max_delay = (base_delay + depth + 1.0) as usize;

        let lfo = Oscillator::new(Waveform::Sine, rate.max(0.01), sample_rate)?;

        Ok(Self {
            delay_line: DelayLine::new(max_delay),
            lfo,
            base_delay,
            depth,
            feedback: feedback.clamp(-0.99, 0.99),
            mix: mix.clamp(0.0, 1.0),
            prev_output: 0.0,
        })
    }

    /// Process a single sample through the flanger.
    #[inline]
    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let lfo_val = self.lfo.next_sample();
        let delay = self.base_delay + self.depth * (lfo_val * 0.5 + 0.5);

        let input_with_fb = input + self.feedback * self.prev_output;
        self.delay_line.write(input_with_fb);

        let delayed = self.delay_line.read(delay);
        self.prev_output = delayed;

        input * (1.0 - self.mix) + delayed * self.mix
    }
}

/// Phaser effect — cascade of allpass filters with LFO modulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Phaser {
    /// Allpass filter states (pairs of z-1 values).
    #[serde(skip)]
    allpass_states: Vec<f32>,
    /// LFO oscillator.
    lfo: Oscillator,
    /// Number of allpass stages (typically 4-12).
    num_stages: usize,
    /// Minimum frequency in Hz.
    min_freq: f32,
    /// Maximum frequency in Hz.
    max_freq: f32,
    /// Feedback coefficient.
    pub feedback: f32,
    /// Wet/dry mix (0.0 to 1.0).
    pub mix: f32,
    /// Sample rate.
    sample_rate: f32,
    /// Previous output for feedback.
    #[serde(skip)]
    prev_output: f32,
}

impl Phaser {
    /// Create a new phaser effect.
    ///
    /// # Errors
    ///
    /// Returns error if sample_rate or LFO rate is invalid.
    pub fn new(
        num_stages: usize,
        rate: f32,
        min_freq: f32,
        max_freq: f32,
        feedback: f32,
        mix: f32,
        sample_rate: f32,
    ) -> Result<Self> {
        if let Some(e) = error::validate_sample_rate(sample_rate) {
            return Err(e);
        }

        // Validate frequency range: must be positive and below Nyquist
        let nyquist = sample_rate * 0.5;
        let min_f = min_freq.max(20.0).min(nyquist);
        let max_f = max_freq.max(min_f).min(nyquist);

        let stages = num_stages.max(2);
        let lfo = Oscillator::new(Waveform::Sine, rate.max(0.01), sample_rate)?;

        Ok(Self {
            allpass_states: vec![0.0; stages],
            lfo,
            num_stages: stages,
            min_freq: min_f,
            max_freq: max_f,
            feedback: feedback.clamp(-0.99, 0.99),
            mix: mix.clamp(0.0, 1.0),
            sample_rate,
            prev_output: 0.0,
        })
    }

    /// Process a single sample through the phaser.
    #[inline]
    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let lfo_val = self.lfo.next_sample();
        // Map LFO to frequency range (logarithmic)
        let t = (lfo_val + 1.0) * 0.5; // 0..1
        let freq = self.min_freq * (self.max_freq / self.min_freq).powf(t);

        // Compute allpass coefficient from frequency (bilinear transform)
        let w = (std::f32::consts::PI * freq / self.sample_rate).min(0.99);
        let coeff = (1.0 - w) / (1.0 + w);

        let mut output = input + self.feedback * self.prev_output;
        for state in &mut self.allpass_states {
            let x = output;
            output = *state - coeff * x;
            *state = coeff * output + x;
        }

        self.prev_output = output;

        input * (1.0 - self.mix) + (input + output) * 0.5 * self.mix
    }
}

/// Distortion effect with multiple distortion types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Distortion {
    /// Type of distortion.
    pub distortion_type: DistortionType,
    /// Drive/gain amount (1.0 = unity).
    pub drive: f32,
    /// Wet/dry mix (0.0 to 1.0).
    pub mix: f32,
}

impl Distortion {
    /// Create a new distortion effect.
    #[must_use]
    pub fn new(distortion_type: DistortionType, drive: f32, mix: f32) -> Self {
        Self {
            distortion_type,
            drive: drive.max(0.0),
            mix: mix.clamp(0.0, 1.0),
        }
    }

    /// Process a single sample through the distortion.
    #[inline]
    #[must_use]
    pub fn process_sample(&self, input: f32) -> f32 {
        let driven = input * self.drive;

        let distorted = match self.distortion_type {
            DistortionType::SoftClip => driven.tanh(),
            DistortionType::HardClip => driven.clamp(-1.0, 1.0),
            DistortionType::WaveFold => {
                // Analytical triangle-wave folding: maps any finite value into -1..1
                // by reflecting at the boundaries. Handles NaN/Inf safely.
                if !driven.is_finite() {
                    0.0
                } else {
                    // Shift so fold boundaries align: x in [0, 4) maps to triangle
                    let x = driven + 1.0; // shift range so -1 maps to 0
                    let period = 4.0_f32;
                    let t = x.rem_euclid(period); // always in [0, 4)
                    if t < 2.0 {
                        t - 1.0 // rising: 0->-1, 1->0, 2->1
                    } else {
                        3.0 - t // falling: 2->1, 3->0, 4->-1
                    }
                }
            }
        };

        input * (1.0 - self.mix) + distorted * self.mix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chorus() {
        let mut chorus = Chorus::new(3, 10.0, 3.0, 0.5, 0.5, 44100.0).unwrap();
        let out = chorus.process_sample(1.0);
        assert!(out.is_finite());
    }

    #[test]
    fn test_flanger() {
        let mut flanger = Flanger::new(2.0, 1.0, 0.5, 0.5, 0.5, 44100.0).unwrap();
        let out = flanger.process_sample(1.0);
        assert!(out.is_finite());
    }

    #[test]
    fn test_phaser() {
        let mut phaser = Phaser::new(4, 0.5, 200.0, 2000.0, 0.5, 0.5, 44100.0).unwrap();
        for _ in 0..100 {
            let out = phaser.process_sample(1.0);
            assert!(out.is_finite(), "phaser produced non-finite output");
        }
    }

    #[test]
    fn test_soft_clip() {
        let dist = Distortion::new(DistortionType::SoftClip, 10.0, 1.0);
        let out = dist.process_sample(0.5);
        assert!(out.abs() <= 1.0, "soft clip should saturate, got {out}");
    }

    #[test]
    fn test_hard_clip() {
        let dist = Distortion::new(DistortionType::HardClip, 10.0, 1.0);
        let out = dist.process_sample(0.5);
        assert!(
            (out - 1.0).abs() < f32::EPSILON,
            "hard clip of 5.0 should be 1.0, got {out}"
        );
    }

    #[test]
    fn test_wave_fold() {
        let dist = Distortion::new(DistortionType::WaveFold, 3.0, 1.0);
        let out = dist.process_sample(0.5);
        assert!(
            out.abs() <= 1.0,
            "wave fold should stay in range, got {out}"
        );
    }

    #[test]
    fn test_mix_dry() {
        let dist = Distortion::new(DistortionType::HardClip, 10.0, 0.0);
        let out = dist.process_sample(0.5);
        assert!(
            (out - 0.5).abs() < f32::EPSILON,
            "mix 0.0 should be fully dry, got {out}"
        );
    }

    #[test]
    fn test_serde_roundtrip() {
        let dist = Distortion::new(DistortionType::SoftClip, 2.0, 0.7);
        let json = serde_json::to_string(&dist).unwrap();
        let back: Distortion = serde_json::from_str(&json).unwrap();
        assert_eq!(dist.distortion_type, back.distortion_type);
        assert!((dist.drive - back.drive).abs() < f32::EPSILON);
    }
}
