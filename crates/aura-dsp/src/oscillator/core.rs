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
    /// Band-limited sawtooth wave (`PolyBLEP`).
    Saw,
    /// Band-limited square wave (`PolyBLEP`).
    Square,
    /// Triangle wave (integrated square).
    Triangle,
    /// Band-limited pulse wave with variable width (`PolyBLEP`).
    Pulse,
    /// White noise.
    WhiteNoise,
    /// Pink noise (Voss-McCartney).
    PinkNoise,
    /// Brown noise (integrated white).
    BrownNoise,
}

/// 4-point `PolyBLEP` correction for anti-aliased discontinuities.
///
/// Extends the correction window to 2 samples on each side of the
/// discontinuity (vs 1 sample for 2-point `PolyBLEP`), providing better
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
    /// Returns `DspError::InvalidSampleRate` if `sample_rate` <= 0.
    /// Returns `DspError::InvalidFrequency` if frequency is out of range
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

    /// Host sample-rate constructor. Clamps invalid `sample_rate` / `frequency`
    /// so plugin `init` does not have to unwrap [`Self::new`].
    ///
    /// # Panics
    ///
    /// Only if clamping failed to satisfy [`Self::new`] — that is a programming
    /// error in this constructor, not a caller input.
    #[must_use]
    pub fn from_host(waveform: Waveform, frequency: f32, sample_rate: f32) -> Self {
        let sr = if sample_rate.is_finite() && sample_rate > 0.0 {
            sample_rate
        } else {
            44_100.0
        };
        let nyquist = sr * 0.5;
        let hz = if frequency.is_finite() {
            frequency.clamp(1e-3, (nyquist - 1e-3).max(1e-3))
        } else {
            1e-3
        };
        Self::new(waveform, hz, sr).expect("from_host clamps into valid range")
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

    /// Sample waveform at `phase + offset_cycles` without advancing state.
    ///
    /// Does **not**:
    /// - advance [`Self::phase`]
    /// - mutate `triangle_sum` (stateless triangle shape, not the leaky integrator)
    /// - consume the noise RNG (noise waveforms return `0.0`)
    ///
    /// `PolyBLEP` uses lookup phase `t = (phase + offset).rem_euclid(1)` with
    /// this oscillator's [`Self::phase_increment`] as `dt`. Non-finite
    /// `offset_cycles` is treated as `0.0`. Pulse uses this oscillator's
    /// pulse width.
    ///
    /// Lookup is not a substitute for [`Self::next_sample`]: callers typically
    /// take a modulated peek, then advance with `next_sample`.
    #[inline]
    #[must_use]
    pub fn sample_offset(&self, offset_cycles: f32) -> f32 {
        let offset = if offset_cycles.is_finite() {
            offset_cycles
        } else {
            0.0
        };
        let t = (self.phase + offset).rem_euclid(1.0);
        let dt = self.phase_increment();
        match self.waveform {
            Waveform::Pulse => self.pulse_at(t, dt),
            other => stateless_waveform_sample(other, t, dt),
        }
    }

    /// Band-limited pulse at phase `t` using this oscillator's pulse width.
    #[inline]
    fn pulse_at(&self, t: f32, dt: f32) -> f32 {
        let pw = self.pulse_width.clamp(0.01, 0.99);
        let naive = if t < pw { 1.0 } else { -1.0 };
        naive + polyblep(t, dt) - polyblep((t + (1.0 - pw)) % 1.0, dt)
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

            Waveform::Pulse => self.pulse_at(t, dt),

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
    /// Returns `DspError::InvalidFrequency` if frequency is out of valid range.
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

    #[test]
    fn from_host_matches_new_on_valid_input() {
        let a = Oscillator::from_host(Waveform::Saw, 440.0, 48_000.0);
        let b = Oscillator::new(Waveform::Saw, 440.0, 48_000.0).unwrap();
        assert_eq!(a.frequency(), b.frequency());
        assert_eq!(a.sample_rate(), b.sample_rate());
    }

    #[test]
    fn from_host_clamps_bad_sr_and_nyquist() {
        let a = Oscillator::from_host(Waveform::Sine, 440.0, 0.0);
        assert!(a.sample_rate() > 0.0);
        let mut osc = Oscillator::from_host(Waveform::Sine, 30_000.0, 44_100.0);
        assert!(osc.next_sample().is_finite());
        assert!(osc.frequency() < 22_050.0);
    }

    #[test]
    fn sample_offset_sine_zero_matches_next_without_advance() {
        let mut osc = Oscillator::from_host(Waveform::Sine, 440.0, 48_000.0);
        osc.set_phase(0.37);
        let phase_before = osc.phase();
        let peek = osc.sample_offset(0.0);
        assert_eq!(osc.phase(), phase_before);
        let advanced = osc.next_sample();
        assert!((peek - advanced).abs() < f32::EPSILON);
        assert_ne!(osc.phase(), phase_before);
    }

    #[test]
    fn sample_offset_does_not_perturb_phase_sequence() {
        let mut with_lookup = Oscillator::from_host(Waveform::Saw, 220.0, 48_000.0);
        with_lookup.set_phase(0.12);
        let mut dry = with_lookup.clone();

        let phase_before = with_lookup.phase();
        let _ = with_lookup.sample_offset(0.33);
        assert_eq!(with_lookup.phase(), phase_before);

        let a = with_lookup.next_sample();
        let b = dry.next_sample();
        assert!((a - b).abs() < f32::EPSILON);
        assert!((with_lookup.phase() - dry.phase()).abs() < f32::EPSILON);
    }

    #[test]
    fn sample_offset_saw_square_offset_changes_sample_keeps_phase() {
        for wf in [Waveform::Saw, Waveform::Square] {
            let mut osc = Oscillator::from_host(wf, 330.0, 48_000.0);
            // 0.4 vs 0.4+0.25 crosses square half-cycle and moves saw amplitude.
            osc.set_phase(0.4);
            let phase = osc.phase();
            let at0 = osc.sample_offset(0.0);
            let at_quarter = osc.sample_offset(0.25);
            assert_eq!(osc.phase(), phase);
            assert!(
                (at0 - at_quarter).abs() > 1e-3,
                "{wf:?}: offset must change sample ({at0} vs {at_quarter})"
            );
        }
    }

    #[test]
    fn sample_offset_triangle_lookups_leave_integrator_path_intact() {
        let mut with_lookup = Oscillator::from_host(Waveform::Triangle, 110.0, 48_000.0);
        with_lookup.set_phase(0.4);
        // Warm the leaky integrator a bit so state is non-trivial.
        for _ in 0..32 {
            let _ = with_lookup.next_sample();
        }
        let mut dry = with_lookup.clone();

        let peek_a = with_lookup.sample_offset(0.1);
        let peek_b = with_lookup.sample_offset(0.7);
        assert!(peek_a.is_finite() && peek_b.is_finite());
        let a = with_lookup.next_sample();
        let b = dry.next_sample();
        assert!(
            (a - b).abs() < f32::EPSILON,
            "triangle lookup must not touch triangle_sum ({a} vs {b})"
        );
        assert!((with_lookup.phase() - dry.phase()).abs() < f32::EPSILON);
    }

    #[test]
    fn sample_offset_non_finite_is_finite_and_keeps_phase() {
        let mut osc = Oscillator::from_host(Waveform::Sine, 440.0, 48_000.0);
        osc.set_phase(0.55);
        let phase = osc.phase();
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let s = osc.sample_offset(bad);
            assert!(s.is_finite(), "offset {bad:?} must yield finite sample");
            assert_eq!(osc.phase(), phase);
        }
    }

    #[test]
    fn sample_offset_from_host_set_phase_roundtrip() {
        let mut osc = Oscillator::from_host(Waveform::Pulse, 100.0, 44_100.0);
        osc.set_phase(0.82);
        assert!((osc.phase() - 0.82).abs() < 1e-6);
        let s = osc.sample_offset(0.0);
        assert!(s.is_finite());
        assert!((osc.phase() - 0.82).abs() < 1e-6);
    }

    #[test]
    fn sample_offset_noise_is_silent_without_rng_consume() {
        let mut osc = Oscillator::from_host(Waveform::WhiteNoise, 0.1, 48_000.0);
        let peek = osc.sample_offset(0.0);
        assert_eq!(peek, 0.0);
        let a = osc.next_sample();
        let mut other = Oscillator::from_host(Waveform::WhiteNoise, 0.1, 48_000.0);
        let b = other.next_sample();
        assert!((a - b).abs() < f32::EPSILON);
    }

    #[test]
    fn sample_offset_negative_wraps_like_positive_equivalent() {
        let mut osc = Oscillator::from_host(Waveform::Saw, 440.0, 48_000.0);
        osc.set_phase(0.4);
        let phase = osc.phase();
        let neg = osc.sample_offset(-0.25);
        let pos = osc.sample_offset(0.75);
        assert_eq!(osc.phase(), phase);
        assert!((neg - pos).abs() < f32::EPSILON);
    }

    #[test]
    fn sample_offset_pulse_uses_oscillator_pulse_width() {
        let mut narrow = Oscillator::from_host(Waveform::Pulse, 220.0, 48_000.0);
        narrow.set_pulse_width(0.25);
        // 0.35 is high at 50% duty, low at 25% duty.
        narrow.set_phase(0.35);
        let mut dry = narrow.clone();
        let mut fifty = Oscillator::from_host(Waveform::Pulse, 220.0, 48_000.0);
        fifty.set_pulse_width(0.5);
        fifty.set_phase(0.35);

        let phase = narrow.phase();
        let peek = narrow.sample_offset(0.0);
        assert_eq!(narrow.phase(), phase);
        let advanced = dry.next_sample();
        assert!((peek - advanced).abs() < f32::EPSILON);
        let fifty_peek = fifty.sample_offset(0.0);
        assert!(
            (peek - fifty_peek).abs() > 1e-3,
            "25% pulse must differ from 50% at t=0.35 ({peek} vs {fifty_peek})"
        );
    }
}
