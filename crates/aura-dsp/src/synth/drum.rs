//! Analog drum synthesis models.
//!
//! Provides kick drum, snare drum, and hi-hat synthesis using
//! pitch-swept oscillators, noise bursts, and decay envelopes.
//! No samples required — all sounds are generated from primitives.

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::filter::{BiquadFilter, FilterType};

/// Kick drum synthesiser: pitch-swept sine body + noise click transient.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KickDrum {
    /// Starting pitch of the body sweep (Hz).
    start_freq: f32,
    /// Ending (resting) pitch of the body sweep (Hz).
    end_freq: f32,
    /// Body oscillator phase (0..1).
    phase: f32,
    /// Current pitch envelope value (0..1), decays toward 0.
    pitch_env: f32,
    /// Body amplitude envelope (0..1), decays toward 0.
    body_amp: f32,
    /// Click amplitude envelope (0..1), decays toward 0.
    click_amp: f32,
    /// Body decay coefficient per sample.
    body_decay: f32,
    /// Click decay coefficient per sample.
    click_decay: f32,
    /// Click noise level.
    click_level: f32,
    /// Pitch envelope decay coefficient per sample.
    pitch_decay: f32,
    /// Simple noise state for click.
    noise_state: u32,
    /// Sample rate in Hz.
    sample_rate: f32,
    /// Whether the drum is currently active.
    active: bool,
}

impl KickDrum {
    /// Create a new kick drum synthesiser.
    ///
    /// # Arguments
    ///
    /// * `start_freq` - Initial frequency of the body sweep (e.g., 150 Hz)
    /// * `end_freq` - Resting frequency after sweep (e.g., 50 Hz)
    /// * `body_decay_ms` - Body decay time in milliseconds
    /// * `click_level` - Click transient level (0.0 to 1.0)
    /// * `sample_rate` - Sample rate in Hz
    ///
    /// # Errors
    ///
    /// Returns error if sample_rate is invalid.
    pub fn new(
        start_freq: f32,
        end_freq: f32,
        body_decay_ms: f32,
        click_level: f32,
        sample_rate: f32,
    ) -> Result<Self> {
        if sample_rate <= 0.0 || !sample_rate.is_finite() {
            return Err(crate::error::NaadError::InvalidSampleRate { sample_rate });
        }

        let body_decay_samples = (body_decay_ms / 1000.0) * sample_rate;
        let body_decay = if body_decay_samples > 0.0 {
            (-6.9 / body_decay_samples).exp()
        } else {
            0.0
        };

        // Click decays much faster than body.
        let click_decay_samples = 0.005 * sample_rate; // 5ms
        let click_decay = if click_decay_samples > 0.0 {
            (-6.9 / click_decay_samples).exp()
        } else {
            0.0
        };

        // Pitch envelope decays over ~30ms.
        let pitch_decay_samples = 0.03 * sample_rate;
        let pitch_decay = if pitch_decay_samples > 0.0 {
            (-6.9 / pitch_decay_samples).exp()
        } else {
            0.0
        };

        Ok(Self {
            start_freq,
            end_freq,
            phase: 0.0,
            pitch_env: 0.0,
            body_amp: 0.0,
            click_amp: 0.0,
            body_decay,
            click_decay,
            click_level: click_level.clamp(0.0, 1.0),
            pitch_decay,
            noise_state: 12345,
            sample_rate,
            active: false,
        })
    }

    /// Trigger the kick drum.
    pub fn trigger(&mut self) {
        self.phase = 0.0;
        self.pitch_env = 1.0;
        self.body_amp = 1.0;
        self.click_amp = 1.0;
        self.active = true;
    }

    /// Generate the next sample.
    #[inline]
    #[must_use]
    pub fn next_sample(&mut self) -> f32 {
        if !self.active {
            return 0.0;
        }

        // Pitch sweep: interpolate from start_freq to end_freq.
        let freq = self.end_freq + (self.start_freq - self.end_freq) * self.pitch_env;
        self.pitch_env *= self.pitch_decay;
        self.pitch_env = crate::flush_denormal(self.pitch_env);

        // Body: sine oscillator.
        let body = (self.phase * std::f32::consts::TAU).sin() * self.body_amp;
        let phase_inc = freq / self.sample_rate;
        self.phase += phase_inc;
        self.phase -= self.phase.floor();

        self.body_amp *= self.body_decay;
        self.body_amp = crate::flush_denormal(self.body_amp);

        // Click: noise burst.
        let noise = self.next_noise();
        let click = noise * self.click_amp * self.click_level;
        self.click_amp *= self.click_decay;
        self.click_amp = crate::flush_denormal(self.click_amp);

        // Deactivate when both envelopes are negligible.
        if self.body_amp < 1e-6 && self.click_amp < 1e-6 {
            self.active = false;
        }

        body + click
    }

    /// Fill a buffer with samples.
    #[inline]
    pub fn fill_buffer(&mut self, buffer: &mut [f32]) {
        for s in buffer.iter_mut() {
            *s = self.next_sample();
        }
    }

    /// Check if the drum is currently producing output.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Signed-`f32` noise sample for the click transient.
    #[inline]
    fn next_noise(&mut self) -> f32 {
        crate::dsp_util::xorshift32_signed_f32(&mut self.noise_state)
    }
}

/// Snare drum synthesiser: sine tone body + bandpass-filtered noise.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnareDrum {
    /// Tone oscillator phase.
    phase: f32,
    /// Tone frequency (Hz).
    tone_freq: f32,
    /// Tone amplitude envelope.
    tone_amp: f32,
    /// Tone decay coefficient.
    tone_decay: f32,
    /// Noise amplitude envelope.
    noise_amp: f32,
    /// Noise decay coefficient.
    noise_decay: f32,
    /// Noise level relative to tone.
    noise_level: f32,
    /// Bandpass filter for noise.
    noise_filter: BiquadFilter,
    /// Noise PRNG state.
    noise_state: u32,
    /// Sample rate in Hz.
    sample_rate: f32,
    /// Whether the drum is currently active.
    active: bool,
}

impl SnareDrum {
    /// Create a new snare drum synthesiser.
    ///
    /// # Errors
    ///
    /// Returns error if sample_rate is invalid.
    pub fn new(sample_rate: f32) -> Result<Self> {
        if sample_rate <= 0.0 || !sample_rate.is_finite() {
            return Err(crate::error::NaadError::InvalidSampleRate { sample_rate });
        }

        let tone_decay_samples = 0.1 * sample_rate; // 100ms
        let tone_decay = (-6.9 / tone_decay_samples).exp();

        let noise_decay_samples = 0.15 * sample_rate; // 150ms
        let noise_decay = (-6.9 / noise_decay_samples).exp();

        let noise_filter = BiquadFilter::new(FilterType::BandPass, sample_rate, 1500.0, 1.5)?;

        Ok(Self {
            phase: 0.0,
            tone_freq: 200.0,
            tone_amp: 0.0,
            tone_decay,
            noise_amp: 0.0,
            noise_decay,
            noise_level: 0.8,
            noise_filter,
            noise_state: 67890,
            sample_rate,
            active: false,
        })
    }

    /// Trigger the snare drum.
    pub fn trigger(&mut self) {
        self.phase = 0.0;
        self.tone_amp = 1.0;
        self.noise_amp = 1.0;
        self.active = true;
    }

    /// Generate the next sample.
    #[inline]
    #[must_use]
    pub fn next_sample(&mut self) -> f32 {
        if !self.active {
            return 0.0;
        }

        // Tone component: sine at ~200 Hz.
        let tone = (self.phase * std::f32::consts::TAU).sin() * self.tone_amp;
        let phase_inc = self.tone_freq / self.sample_rate;
        self.phase += phase_inc;
        self.phase -= self.phase.floor();
        self.tone_amp *= self.tone_decay;
        self.tone_amp = crate::flush_denormal(self.tone_amp);

        // Noise component: bandpass-filtered white noise.
        let noise_raw = self.next_noise();
        let noise_filtered = self.noise_filter.process_sample(noise_raw);
        let noise = noise_filtered * self.noise_amp * self.noise_level;
        self.noise_amp *= self.noise_decay;
        self.noise_amp = crate::flush_denormal(self.noise_amp);

        if self.tone_amp < 1e-6 && self.noise_amp < 1e-6 {
            self.active = false;
        }

        tone + noise
    }

    /// Fill a buffer with samples.
    #[inline]
    pub fn fill_buffer(&mut self, buffer: &mut [f32]) {
        for s in buffer.iter_mut() {
            *s = self.next_sample();
        }
    }

    /// Check if the drum is currently producing output.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Signed-`f32` noise sample for the snare body.
    #[inline]
    fn next_noise(&mut self) -> f32 {
        crate::dsp_util::xorshift32_signed_f32(&mut self.noise_state)
    }
}

/// Hi-hat synthesiser: detuned square oscillators through highpass + bandpass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiHat {
    /// Phases for the metallic oscillators.
    phases: [f32; 6],
    /// Frequencies for the metallic oscillators (detuned, inharmonic).
    frequencies: [f32; 6],
    /// Amplitude envelope.
    amp: f32,
    /// Decay coefficient.
    decay: f32,
    /// Highpass filter.
    highpass: BiquadFilter,
    /// Bandpass filter.
    bandpass: BiquadFilter,
    /// Sample rate in Hz.
    sample_rate: f32,
    /// Whether the hat is currently active.
    active: bool,
}

impl HiHat {
    /// Create a new hi-hat synthesiser.
    ///
    /// # Arguments
    ///
    /// * `open` - If true, use a longer decay (open hi-hat).
    /// * `sample_rate` - Sample rate in Hz.
    ///
    /// # Errors
    ///
    /// Returns error if sample_rate is invalid.
    pub fn new(open: bool, sample_rate: f32) -> Result<Self> {
        if sample_rate <= 0.0 || !sample_rate.is_finite() {
            return Err(crate::error::NaadError::InvalidSampleRate { sample_rate });
        }

        let decay_ms = if open { 200.0 } else { 30.0 };
        let decay_samples = (decay_ms / 1000.0) * sample_rate;
        let decay = (-6.9 / decay_samples).exp();

        // Inharmonic metallic frequencies (based on classic 808 ratios).
        let frequencies = [205.3, 304.4, 369.6, 522.7, 540.5, 800.6];

        let highpass = BiquadFilter::new(FilterType::HighPass, sample_rate, 6000.0, 0.707)?;
        let bandpass = BiquadFilter::new(FilterType::BandPass, sample_rate, 10000.0, 1.0)?;

        Ok(Self {
            phases: [0.0; 6],
            frequencies,
            amp: 0.0,
            decay,
            highpass,
            bandpass,
            sample_rate,
            active: false,
        })
    }

    /// Trigger the hi-hat.
    pub fn trigger(&mut self) {
        self.phases = [0.0; 6];
        self.amp = 1.0;
        self.active = true;
    }

    /// Generate the next sample.
    #[inline]
    #[must_use]
    pub fn next_sample(&mut self) -> f32 {
        if !self.active {
            return 0.0;
        }

        // Sum of detuned square oscillators for metallic tone.
        let mut metallic = 0.0f32;
        for i in 0..6 {
            let sq = if self.phases[i] < 0.5 { 1.0 } else { -1.0 };
            metallic += sq;
            let inc = self.frequencies[i] / self.sample_rate;
            self.phases[i] += inc;
            self.phases[i] -= self.phases[i].floor();
        }
        metallic /= 6.0;

        // Filter chain: highpass -> bandpass.
        let hp = self.highpass.process_sample(metallic);
        let bp = self.bandpass.process_sample(hp);
        let out = bp * self.amp;

        self.amp *= self.decay;
        self.amp = crate::flush_denormal(self.amp);

        if self.amp < 1e-6 {
            self.active = false;
        }

        out
    }

    /// Fill a buffer with samples.
    #[inline]
    pub fn fill_buffer(&mut self, buffer: &mut [f32]) {
        for s in buffer.iter_mut() {
            *s = self.next_sample();
        }
    }

    /// Check if the hi-hat is currently producing output.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kick_produces_output_and_decays() {
        let mut kick = KickDrum::new(150.0, 50.0, 200.0, 0.5, 44100.0).unwrap();
        kick.trigger();
        assert!(kick.is_active());

        let mut buf = [0.0f32; 512];
        kick.fill_buffer(&mut buf);
        assert!(
            buf.iter().any(|&s| s.abs() > 0.01),
            "kick should produce output"
        );
        assert!(buf.iter().all(|s| s.is_finite()));

        // Run until decay.
        for _ in 0..200 {
            let mut decay_buf = [0.0f32; 512];
            kick.fill_buffer(&mut decay_buf);
        }
        assert!(!kick.is_active(), "kick should decay to silence");
    }

    #[test]
    fn test_snare_produces_output_and_decays() {
        let mut snare = SnareDrum::new(44100.0).unwrap();
        snare.trigger();
        assert!(snare.is_active());

        let mut buf = [0.0f32; 512];
        snare.fill_buffer(&mut buf);
        assert!(
            buf.iter().any(|&s| s.abs() > 0.001),
            "snare should produce output"
        );
        assert!(buf.iter().all(|s| s.is_finite()));

        for _ in 0..200 {
            let mut decay_buf = [0.0f32; 512];
            snare.fill_buffer(&mut decay_buf);
        }
        assert!(!snare.is_active(), "snare should decay to silence");
    }

    #[test]
    fn test_hihat_produces_output_and_decays() {
        let mut hat = HiHat::new(false, 44100.0).unwrap();
        hat.trigger();
        assert!(hat.is_active());

        let mut buf = [0.0f32; 512];
        hat.fill_buffer(&mut buf);
        assert!(
            buf.iter().any(|&s| s.abs() > 0.0001),
            "hihat should produce output"
        );
        assert!(buf.iter().all(|s| s.is_finite()));

        for _ in 0..200 {
            let mut decay_buf = [0.0f32; 512];
            hat.fill_buffer(&mut decay_buf);
        }
        assert!(!hat.is_active(), "hihat should decay to silence");
    }

    #[test]
    fn test_kick_serde_roundtrip() {
        let kick = KickDrum::new(150.0, 50.0, 200.0, 0.5, 44100.0).unwrap();
        let json = serde_json::to_string(&kick).unwrap();
        let back: KickDrum = serde_json::from_str(&json).unwrap();
        assert!((kick.start_freq - back.start_freq).abs() < f32::EPSILON);
        assert!((kick.end_freq - back.end_freq).abs() < f32::EPSILON);
    }

    #[test]
    fn test_snare_serde_roundtrip() {
        let snare = SnareDrum::new(44100.0).unwrap();
        let json = serde_json::to_string(&snare).unwrap();
        let back: SnareDrum = serde_json::from_str(&json).unwrap();
        assert!((snare.tone_freq - back.tone_freq).abs() < f32::EPSILON);
    }

    #[test]
    fn test_hihat_serde_roundtrip() {
        let hat = HiHat::new(false, 44100.0).unwrap();
        let json = serde_json::to_string(&hat).unwrap();
        let back: HiHat = serde_json::from_str(&json).unwrap();
        assert!((hat.frequencies[0] - back.frequencies[0]).abs() < f32::EPSILON);
    }
}
