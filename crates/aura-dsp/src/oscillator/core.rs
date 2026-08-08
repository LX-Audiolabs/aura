//! Core oscillator: [`Waveform`], [`Oscillator`], and the [`polyblep`] kernel.

use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::error::{self, Result};
use crate::noise;

/// Waveform type for an oscillator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Waveform {
    /// Sine wave.
    Sine,
    /// Band-limited sawtooth wave (PolyBLEP).
    Saw,
    /// Band-limited square wave (PolyBLEP).
    Square,
    /// Triangle wave (integrated square).
    Triangle,
    /// Band-limited pulse wave with variable width (PolyBLEP).
    Pulse,
    /// White noise.
    WhiteNoise,
    /// Pink noise (Voss-McCartney).
    PinkNoise,
    /// Brown noise (integrated white).
    BrownNoise,
}

/// 4-point PolyBLEP correction for anti-aliased discontinuities.
///
/// Extends the correction window to 2 samples on each side of the
/// discontinuity (vs 1 sample for 2-point PolyBLEP), providing better
/// suppression of aliasing harmonics at high frequencies. The residual
/// is derived from an integrated piecewise-cubic BLAMP kernel, yielding
/// C1 continuity at the transition boundaries.
///
/// `t` is the phase position (0..1), `dt` is the phase increment per sample.
#[inline]
#[must_use]
pub fn polyblep(t: f32, dt: f32) -> f32 {
    if dt <= 0.0 {
        return 0.0;
    }
    let dt2 = 2.0 * dt;
    if t < dt {
        // First sample after discontinuity
        let n = t / dt;
        let n2 = n * n;
        let blep2 = 2.0 * n - n2 - 1.0;
        let cubic = n2 * (n - 1.0) * 0.5;
        blep2 + cubic
    } else if t < dt2 {
        // Second sample after discontinuity (cubic tail)
        let n = t / dt - 1.0;
        let n2 = n * n;
        -n2 * (1.0 - n) * 0.5
    } else if t > 1.0 - dt {
        // First sample before discontinuity
        let n = (t - 1.0) / dt;
        let n2 = n * n;
        let blep2 = n2 + 2.0 * n + 1.0;
        let cubic = -n2 * (n + 1.0) * 0.5;
        blep2 + cubic
    } else if t > 1.0 - dt2 {
        // Second sample before discontinuity (cubic tail)
        let n = (t - 1.0) / dt + 1.0;
        let n2 = n * n;
        n2 * (1.0 + n) * 0.5
    } else {
        0.0
    }
}

/// Audio oscillator with band-limited waveform generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Oscillator {
    waveform: Waveform,
    frequency: f32,
    phase: f32,
    sample_rate: f32,
    pulse_width: f32,
    #[serde(skip)]
    noise_gen: Option<noise::NoiseGenerator>,
    #[serde(skip)]
    triangle_sum: f32,
}

impl Oscillator {
    /// Create a new oscillator.
    ///
    /// # Errors
    ///
    /// Returns `NaadError::InvalidSampleRate` if sample_rate <= 0.
    /// Returns `NaadError::InvalidFrequency` if frequency is out of range
    /// (does not apply to noise waveforms).
    pub fn new(waveform: Waveform, frequency: f32, sample_rate: f32) -> Result<Self> {
        if let Some(e) = error::validate_sample_rate(sample_rate) {
            return Err(e);
        }

        let is_noise = matches!(
            waveform,
            Waveform::WhiteNoise | Waveform::PinkNoise | Waveform::BrownNoise
        );

        if !is_noise && let Some(e) = error::validate_frequency(frequency, sample_rate) {
            return Err(e);
        }

        let noise_gen = match waveform {
            Waveform::WhiteNoise => Some(noise::NoiseGenerator::new(noise::NoiseType::White, 42)),
            Waveform::PinkNoise => Some(noise::NoiseGenerator::new(noise::NoiseType::Pink, 42)),
            Waveform::BrownNoise => Some(noise::NoiseGenerator::new(noise::NoiseType::Brown, 42)),
            _ => None,
        };

        debug!(?waveform, frequency, sample_rate, "oscillator created");
        Ok(Self {
            waveform,
            frequency,
            phase: 0.0,
            sample_rate,
            pulse_width: 0.5,
            noise_gen,
            triangle_sum: 0.0,
        })
    }

    /// Phase increment per sample.
    #[inline]
    #[must_use]
    pub fn phase_increment(&self) -> f32 {
        self.frequency / self.sample_rate
    }

    /// Ensure internal state is initialized (recovers after deserialization).
    fn ensure_initialized(&mut self) {
        if self.noise_gen.is_none() {
            self.noise_gen = match self.waveform {
                Waveform::WhiteNoise => {
                    Some(noise::NoiseGenerator::new(noise::NoiseType::White, 42))
                }
                Waveform::PinkNoise => Some(noise::NoiseGenerator::new(noise::NoiseType::Pink, 42)),
                Waveform::BrownNoise => {
                    Some(noise::NoiseGenerator::new(noise::NoiseType::Brown, 42))
                }
                _ => None,
            };
        }
    }

    /// Generate the next sample.
    #[inline]
    #[must_use]
    pub fn next_sample(&mut self) -> f32 {
        let dt = self.phase_increment();
        let t = self.phase;

        let sample = match self.waveform {
            Waveform::Sine => (t * std::f32::consts::TAU).sin(),

            Waveform::Saw => {
                let naive = 2.0 * t - 1.0;
                naive - polyblep(t, dt)
            }

            Waveform::Square => {
                let naive = if t < 0.5 { 1.0 } else { -1.0 };
                naive + polyblep(t, dt) - polyblep((t + 0.5) % 1.0, dt)
            }

            Waveform::Triangle => {
                // Integrated square wave for triangle
                let square = if t < 0.5 { 1.0 } else { -1.0 };
                let square_blep = square + polyblep(t, dt) - polyblep((t + 0.5) % 1.0, dt);
                // Leaky integrator
                self.triangle_sum = 0.999 * self.triangle_sum + square_blep * dt * 4.0;
                self.triangle_sum.clamp(-1.0, 1.0)
            }

            Waveform::Pulse => {
                let pw = self.pulse_width.clamp(0.01, 0.99);
                let naive = if t < pw { 1.0 } else { -1.0 };
                naive + polyblep(t, dt) - polyblep((t + (1.0 - pw)) % 1.0, dt)
            }

            Waveform::WhiteNoise | Waveform::PinkNoise | Waveform::BrownNoise => {
                // Lazy init: reconstruct noise_gen after deserialization
                self.ensure_initialized();
                if let Some(ref mut ng) = self.noise_gen {
                    ng.next_sample()
                } else {
                    0.0
                }
            }
        };

        // Advance phase
        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        sample
    }

    /// Fill a buffer with generated samples.
    #[inline]
    pub fn fill_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.next_sample();
        }
    }

    /// Returns the waveform type.
    #[inline]
    #[must_use]
    pub fn waveform(&self) -> Waveform {
        self.waveform
    }

    /// Returns the current frequency in Hz.
    #[inline]
    #[must_use]
    pub fn frequency(&self) -> f32 {
        self.frequency
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

    /// Returns the pulse width (0.0 to 1.0).
    #[inline]
    #[must_use]
    pub fn pulse_width(&self) -> f32 {
        self.pulse_width
    }

    /// Set the oscillator frequency.
    ///
    /// # Errors
    ///
    /// Returns `NaadError::InvalidFrequency` if frequency is out of valid range.
    pub fn set_frequency(&mut self, freq: f32) -> Result<()> {
        if let Some(e) = error::validate_frequency(freq, self.sample_rate) {
            return Err(e);
        }
        self.frequency = freq;
        Ok(())
    }

    /// Set the pulse width (clamped to 0.01..0.99).
    pub fn set_pulse_width(&mut self, pw: f32) {
        self.pulse_width = pw.clamp(0.01, 0.99);
    }

    /// Set the oscillator phase (0.0 to 1.0).
    pub fn set_phase(&mut self, phase: f32) {
        self.phase = phase.rem_euclid(1.0);
    }

    /// Reset the oscillator phase to zero.
    pub fn reset_phase(&mut self) {
        self.phase = 0.0;
        self.triangle_sum = 0.0;
    }

    /// Advance phase by a custom increment (for FM synthesis).
    ///
    /// Returns the sine of the current phase before advancing.
    /// This is used by FM synthesis which needs to control the
    /// instantaneous frequency directly.
    #[inline]
    pub fn advance_phase_sine(&mut self, dt: f32) -> f32 {
        let sample = (self.phase * std::f32::consts::TAU).sin();
        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        sample
    }
}

/// Generate a single waveform sample from phase and phase increment (no state mutation).
///
/// Used by [`super::unison::UnisonOscillator`] to avoid duplicating waveform logic per voice.
/// Does not support noise waveforms or triangle integration (stateless).
#[inline]
pub(super) fn stateless_waveform_sample(waveform: Waveform, t: f32, dt: f32) -> f32 {
    match waveform {
        Waveform::Sine => (t * std::f32::consts::TAU).sin(),
        Waveform::Saw => {
            let naive = 2.0 * t - 1.0;
            naive - polyblep(t, dt)
        }
        Waveform::Square => {
            let naive = if t < 0.5 { 1.0 } else { -1.0 };
            naive + polyblep(t, dt) - polyblep((t + 0.5) % 1.0, dt)
        }
        Waveform::Triangle => {
            if t < 0.25 {
                4.0 * t
            } else if t < 0.75 {
                2.0 - 4.0 * t
            } else {
                4.0 * t - 4.0
            }
        }
        Waveform::Pulse => {
            // Default 50% duty cycle for unison (pulse width not per-voice)
            let naive = if t < 0.5 { 1.0 } else { -1.0 };
            naive + polyblep(t, dt) - polyblep((t + 0.5) % 1.0, dt)
        }
        // Noise waveforms don't make sense for unison detuning — produce silence
        Waveform::WhiteNoise | Waveform::PinkNoise | Waveform::BrownNoise => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sine_range() {
        let mut osc = Oscillator::new(Waveform::Sine, 440.0, 44100.0).unwrap();
        for _ in 0..1000 {
            let s = osc.next_sample();
            assert!((-1.0..=1.0).contains(&s), "sample out of range: {s}");
        }
    }

    #[test]
    fn test_saw_range() {
        let mut osc = Oscillator::new(Waveform::Saw, 440.0, 44100.0).unwrap();
        for _ in 0..1000 {
            let s = osc.next_sample();
            assert!((-1.5..=1.5).contains(&s), "saw sample out of range: {s}");
        }
    }

    #[test]
    fn test_invalid_frequency() {
        assert!(Oscillator::new(Waveform::Sine, -1.0, 44100.0).is_err());
        assert!(Oscillator::new(Waveform::Sine, 0.0, 44100.0).is_err());
        assert!(Oscillator::new(Waveform::Sine, 25000.0, 44100.0).is_err());
    }

    #[test]
    fn test_invalid_sample_rate() {
        assert!(Oscillator::new(Waveform::Sine, 440.0, 0.0).is_err());
        assert!(Oscillator::new(Waveform::Sine, 440.0, -1.0).is_err());
    }

    #[test]
    fn test_set_frequency() {
        let mut osc = Oscillator::new(Waveform::Sine, 440.0, 44100.0).unwrap();
        assert!(osc.set_frequency(880.0).is_ok());
        assert!(osc.set_frequency(0.0).is_err());
    }

    #[test]
    fn test_fill_buffer() {
        let mut osc = Oscillator::new(Waveform::Sine, 440.0, 44100.0).unwrap();
        let mut buf = [0.0f32; 128];
        osc.fill_buffer(&mut buf);
        assert!(buf.iter().any(|&s| s != 0.0));
    }

    #[test]
    fn test_serde_roundtrip() {
        let osc = Oscillator::new(Waveform::Saw, 440.0, 44100.0).unwrap();
        let json = serde_json::to_string(&osc).unwrap();
        let back: Oscillator = serde_json::from_str(&json).unwrap();
        assert_eq!(osc.waveform(), back.waveform());
        assert!((osc.frequency() - back.frequency()).abs() < f32::EPSILON);
    }

    #[test]
    fn test_polyblep_function() {
        assert!((polyblep(0.5, 0.01) - 0.0).abs() < f32::EPSILON);
        assert!(polyblep(0.001, 0.01).abs() > 0.0);
    }

    #[test]
    fn test_noise_waveforms() {
        let mut osc = Oscillator::new(Waveform::WhiteNoise, 0.1, 44100.0).unwrap();
        let s = osc.next_sample();
        assert!(s.is_finite());
    }
}
