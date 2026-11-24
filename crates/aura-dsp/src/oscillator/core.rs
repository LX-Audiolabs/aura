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
    /// Band-limited falling sawtooth wave (`-Saw`).
    SawDown,
    /// Band-limited square wave (`PolyBLEP`).
    Square,
    /// Triangle wave with `PolyBLAMP` slope correction.
    Triangle,
    /// Band-limited pulse wave with variable width (`PolyBLEP`).
    Pulse,
    /// White noise.
    WhiteNoise,
    /// Pink noise (Voss-McCartney).
    PinkNoise,
    /// Brown noise (integrated white).
    BrownNoise,
    /// Three-level stepped waveform.
    Step3,
    /// Four-level stepped waveform.
    Step4,
    /// Eight-level stepped waveform.
    Step8,
    /// Three-point pyramid waveform.
    Pyramid3,
    /// Five-point pyramid waveform.
    Pyramid5,
    /// Nine-point pyramid waveform.
    Pyramid9,
}

/// Two-point `PolyBLEP` residual for a unit phase wrap, valid through Nyquist.
/// `t` is in 0..1; `dt` is the positive phase increment, at most 0.5.
#[inline]
#[must_use]
pub fn polyblep(t: f32, dt: f32) -> f32 {
    if !dt.is_finite() || dt <= 0.0 || !t.is_finite() {
        return 0.0;
    }
    let dt = dt.min(0.5);
    if t < dt {
        let n = t / dt;
        n * (2.0 - n) - 1.0
    } else if t > 1.0 - dt {
        let n = (t - 1.0) / dt;
        (n + 1.0) * (n + 1.0)
    } else {
        0.0
    }
}

/// Integral of the BLEP residual, used at triangle slope discontinuities.
#[inline]
fn polyblamp(t: f32, dt: f32) -> f32 {
    if dt <= 0.0 {
        return 0.0;
    }
    let n = if t < dt {
        1.0 - t / dt
    } else if t > 1.0 - dt {
        1.0 + (t - 1.0) / dt
    } else {
        return 0.0;
    };
    dt * n * n * n / 3.0
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
}

/// Default noise PRNG seed for [`Oscillator::new`] / [`Oscillator::from_host`]
/// and post-serde lazy init. Kept at 42 so existing tests and callers stay
/// bit-identical.
const DEFAULT_NOISE_SEED: u32 = 42;

fn noise_gen_for(waveform: Waveform, seed: u32) -> Option<noise::NoiseGenerator> {
    match waveform {
        Waveform::WhiteNoise => Some(noise::NoiseGenerator::new(noise::NoiseType::White, seed)),
        Waveform::PinkNoise => Some(noise::NoiseGenerator::new(noise::NoiseType::Pink, seed)),
        Waveform::BrownNoise => Some(noise::NoiseGenerator::new(noise::NoiseType::Brown, seed)),
        _ => None,
    }
}

impl Oscillator {
    /// Create a new oscillator (noise waveforms use seed **42**).
    ///
    /// # Errors
    ///
    /// Returns `DspError::InvalidSampleRate` if `sample_rate` <= 0.
    /// Returns `DspError::InvalidFrequency` if frequency is out of range
    /// (does not apply to noise waveforms).
    pub fn new(waveform: Waveform, frequency: f32, sample_rate: f32) -> Result<Self> {
        Self::new_with_seed(waveform, frequency, sample_rate, DEFAULT_NOISE_SEED)
    }

    /// Create a new oscillator with an explicit noise PRNG seed.
    ///
    /// Non-noise waveforms ignore `seed`. For white/pink/brown, `seed == 0` is
    /// treated as `1` (same rule as [`crate::noise::NoiseGenerator::new`]).
    /// Stack-only; no heap alloc beyond what `NoiseGenerator::new` already does.
    ///
    /// # Errors
    ///
    /// Same as [`Self::new`].
    pub fn new_with_seed(
        waveform: Waveform,
        frequency: f32,
        sample_rate: f32,
        seed: u32,
    ) -> Result<Self> {
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

        let noise_gen = noise_gen_for(waveform, seed);

        debug!(
            ?waveform,
            frequency, sample_rate, seed, "oscillator created"
        );
        Ok(Self {
            waveform,
            frequency,
            phase: 0.0,
            sample_rate,
            pulse_width: 0.5,
            noise_gen,
        })
    }

    /// Host sample-rate constructor. Clamps invalid `sample_rate` / `frequency`
    /// so plugin `init` does not have to unwrap [`Self::new`]. Noise seed **42**.
    ///
    /// # Panics
    ///
    /// Only if clamping failed to satisfy [`Self::new`] — that is a programming
    /// error in this constructor, not a caller input.
    #[must_use]
    pub fn from_host(waveform: Waveform, frequency: f32, sample_rate: f32) -> Self {
        Self::from_host_with_seed(waveform, frequency, sample_rate, DEFAULT_NOISE_SEED)
    }

    /// Like [`Self::from_host`], but with an explicit noise PRNG seed.
    ///
    /// Same clamp policy as `from_host`. Non-noise waveforms ignore `seed`.
    /// `seed == 0` → `1` for noise (see [`crate::noise::NoiseGenerator::new`]).
    /// RT-safe after construction (stack state only).
    ///
    /// # Panics
    ///
    /// Only if clamping failed to satisfy [`Self::new_with_seed`] — programming
    /// error in this constructor, not a caller input.
    #[must_use]
    pub fn from_host_with_seed(
        waveform: Waveform,
        frequency: f32,
        sample_rate: f32,
        seed: u32,
    ) -> Self {
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
        Self::new_with_seed(waveform, hz, sr, seed).expect("from_host clamps into valid range")
    }

    /// Rebuild the internal noise generator with `seed` (full state reset).
    ///
    /// White/pink/brown: replaces `noise_gen` via fresh `NoiseGenerator::new`
    /// for the current waveform type (pink octaves / brown leak restart).
    /// Non-noise: no-op (clears any stray `noise_gen`).
    ///
    /// RT-safe: stack-only, same as construction. `seed == 0` → `1`.
    /// After serde, `noise_gen` is skipped — call this if a non-default seed is
    /// required (lazy default rebuild uses seed 42).
    #[inline]
    pub fn reseed(&mut self, seed: u32) {
        self.noise_gen = noise_gen_for(self.waveform, seed);
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
            self.noise_gen = noise_gen_for(self.waveform, DEFAULT_NOISE_SEED);
        }
    }

    /// Sample waveform at `phase + offset_cycles` without advancing state.
    ///
    /// Does **not**:
    /// - advance [`Self::phase`]
    /// - mutate waveform state (triangle uses a stateless `PolyBLAMP` shape)
    /// - consume the noise RNG (noise waveforms return `0.0`)
    ///
    /// `PolyBLEP` uses lookup phase `t = (phase + offset).rem_euclid(1)` with
    /// this oscillator's [`Self::phase_increment`] as `dt`. Non-finite
    /// `offset_cycles` is treated as `0.0`. Pulse uses this oscillator's
    /// pulse width.
    ///
    /// Lookup is not a substitute for [`Self::next_sample`]: callers typically
    /// take a modulated lookup, then advance with `next_sample`.
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

    /// Uncorrected waveform value at an event, for hard-sync jump magnitude.
    pub(super) fn raw_sample_at(&self, phase: f32) -> f32 {
        match self.waveform {
            Waveform::Pulse => {
                if phase < self.pulse_width {
                    1.0
                } else {
                    -1.0
                }
            }
            other => stateless_waveform_sample(other, phase, 0.0),
        }
    }

    /// Discontinuities of the periodic shape (up to sixteen for `Pyramid9`).
    pub(super) fn edges(&self) -> ([f32; 16], usize) {
        let mut edges = [0.0; 16];
        let count = match self.waveform {
            Waveform::Saw | Waveform::SawDown => 1,
            Waveform::Square | Waveform::Pulse => 2,
            Waveform::Step3 => 3,
            Waveform::Step4 | Waveform::Pyramid3 => 4,
            Waveform::Step8 | Waveform::Pyramid5 => 8,
            Waveform::Pyramid9 => 16,
            _ => 0,
        };
        for (i, edge) in edges.iter_mut().take(count).enumerate() {
            *edge = i as f32 / count as f32;
        }
        if self.waveform == Waveform::Pulse {
            edges[1] = self.pulse_width;
        }
        // Pyramids are shifted square sums: every edge lies on this grid.
        if matches!(
            self.waveform,
            Waveform::Pyramid3 | Waveform::Pyramid5 | Waveform::Pyramid9
        ) {
            for edge in edges.iter_mut().take(count) {
                *edge = (*edge + 0.25).rem_euclid(1.0);
            }
        }
        (edges, count)
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

            Waveform::SawDown => {
                let naive = 1.0 - 2.0 * t;
                naive + polyblep(t, dt)
            }

            Waveform::Square => {
                let naive = if t < 0.5 { 1.0 } else { -1.0 };
                naive + polyblep(t, dt) - polyblep((t + 0.5) % 1.0, dt)
            }

            Waveform::Triangle => stateless_waveform_sample(Waveform::Triangle, t, dt),

            Waveform::Pulse => self.pulse_at(t, dt),

            Waveform::Step3
            | Waveform::Step4
            | Waveform::Step8
            | Waveform::Pyramid3
            | Waveform::Pyramid5
            | Waveform::Pyramid9 => stateless_waveform_sample(self.waveform, t, dt),

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
        if pw.is_finite() {
            self.pulse_width = pw.clamp(0.01, 0.99);
        }
    }

    /// Set the oscillator phase (0.0 to 1.0).
    pub fn set_phase(&mut self, phase: f32) {
        if phase.is_finite() {
            self.phase = phase.rem_euclid(1.0);
        }
    }

    /// Reset the oscillator phase to zero.
    pub fn reset_phase(&mut self) {
        self.phase = 0.0;
    }

    /// Advance phase by a custom increment (for FM synthesis).
    ///
    /// Returns the sine of the current phase before advancing.
    /// This is used by FM synthesis which needs to control the
    /// instantaneous frequency directly.
    #[inline]
    pub fn advance_phase_sine(&mut self, dt: f32) -> f32 {
        let sample = (self.phase * std::f32::consts::TAU).sin();
        if dt.is_finite() {
            self.phase = (self.phase + dt).rem_euclid(1.0);
        }
        sample
    }
}

/// Generate a single waveform sample from phase and phase increment (no state mutation).
///
/// Used by [`super::unison::UnisonOscillator`] to avoid duplicating waveform logic per voice.
/// Does not support noise waveforms. Triangle uses the same `PolyBLAMP` shape as the core.
#[inline]
pub(super) fn stateless_waveform_sample(waveform: Waveform, t: f32, dt: f32) -> f32 {
    match waveform {
        Waveform::Sine => (t * std::f32::consts::TAU).sin(),
        Waveform::Saw => {
            let naive = 2.0 * t - 1.0;
            naive - polyblep(t, dt)
        }
        Waveform::SawDown => {
            let naive = 1.0 - 2.0 * t;
            naive + polyblep(t, dt)
        }
        Waveform::Square => {
            let naive = if t < 0.5 { 1.0 } else { -1.0 };
            naive + polyblep(t, dt) - polyblep((t + 0.5) % 1.0, dt)
        }
        Waveform::Triangle => {
            let naive = if t < 0.25 {
                4.0 * t
            } else if t < 0.75 {
                2.0 - 4.0 * t
            } else {
                4.0 * t - 4.0
            };
            naive
                + 4.0
                    * (polyblamp((t + 0.25).rem_euclid(1.0), dt)
                        - polyblamp((t + 0.75).rem_euclid(1.0), dt))
        }

        Waveform::Pulse => {
            // Default 50% duty cycle for unison (pulse width not per-voice)
            let naive = if t < 0.5 { 1.0 } else { -1.0 };
            naive + polyblep(t, dt) - polyblep((t + 0.5) % 1.0, dt)
        }
        Waveform::Step3
        | Waveform::Step4
        | Waveform::Step8
        | Waveform::Pyramid3
        | Waveform::Pyramid5
        | Waveform::Pyramid9 => digital_waveform_sample(waveform, t, dt),
        // Noise waveforms don't make sense for unison detuning — produce silence
        Waveform::WhiteNoise | Waveform::PinkNoise | Waveform::BrownNoise => 0.0,
    }
}

#[inline]
fn digital_waveform_sample(waveform: Waveform, t: f32, dt: f32) -> f32 {
    let (step_count, pyramid_count) = match waveform {
        Waveform::Step3 => (Some(3), None),
        Waveform::Step4 => (Some(4), None),
        Waveform::Step8 => (Some(8), None),
        Waveform::Pyramid3 => (None, Some(3)),
        Waveform::Pyramid5 => (None, Some(5)),
        Waveform::Pyramid9 => (None, Some(9)),
        _ => return 0.0,
    };

    if let Some(count) = step_count {
        let section = (t * count as f32).floor() as usize;
        let level = 2.0 * section as f32 / (count - 1) as f32 - 1.0;
        let mut sample = level - polyblep(t, dt);
        let jump = 1.0 / (count - 1) as f32;
        for edge in 1..count {
            let phase = (t - edge as f32 / count as f32).rem_euclid(1.0);
            sample += jump * polyblep(phase, dt);
        }
        return sample;
    }

    let count = pyramid_count.unwrap_or(1);
    let squares = count - 1;
    (0..squares)
        .map(|i| {
            let phase = (t + 0.75 + i as f32 / (2.0 * squares as f32)).rem_euclid(1.0);
            let naive = if phase < 0.5 { 1.0 } else { -1.0 };
            naive + polyblep(phase, dt) - polyblep((phase + 0.5).rem_euclid(1.0), dt)
        })
        .sum::<f32>()
        / squares as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regression_blep_symmetry_triangle_pitch_and_invalid_controls() {
        for dt in [0.01, 0.1, 0.3, 0.49] {
            for i in 1..1000 {
                let t = i as f32 / 1000.0;
                assert!((polyblep(t, dt) + polyblep(1.0 - t, dt)).abs() < 1e-5);
            }
        }
        for sr in [48_000.0, 96_000.0] {
            let mut osc = Oscillator::new(Waveform::Triangle, 5.0, sr).unwrap();
            let mut peak = 0.0_f32;
            for _ in 0..sr as usize {
                let lookup = osc.sample_offset(0.0);
                let y = osc.next_sample();
                assert!((lookup - y).abs() < 1e-6);
                peak = peak.max(y.abs());
            }
            assert!(peak > 0.99, "triangle bass peak {peak}");
        }
        let mut osc = Oscillator::new(Waveform::Pulse, 440.0, 48_000.0).unwrap();
        osc.set_phase(f32::NAN);
        osc.set_pulse_width(f32::NAN);
        assert!(osc.next_sample().is_finite());
    }

    #[test]
    fn saw_down_is_negative_of_rising_saw_at_lookup_phases() {
        for phase in [0.25, 0.75] {
            let mut up = Oscillator::new(Waveform::Saw, 440.0, 48_000.0).unwrap();
            let mut down = Oscillator::new(Waveform::SawDown, 440.0, 48_000.0).unwrap();
            up.set_phase(phase);
            down.set_phase(phase);
            assert!(up.sample_offset(0.0) * if phase < 0.5 { -1.0 } else { 1.0 } > 0.0);
            assert!((up.sample_offset(0.0) + down.sample_offset(0.0)).abs() < 1e-6);
        }
    }

    #[test]
    fn helm_step_and_pyramid_shapes_match_phase_contract() {
        let steps = [
            (Waveform::Step3, 3),
            (Waveform::Step4, 4),
            (Waveform::Step8, 8),
        ];
        for (shape, count) in steps {
            for level in 0..count {
                let phase = (level as f32 + 0.5) / count as f32;
                let expected = 2.0 * level as f32 / (count - 1) as f32 - 1.0;
                assert!((stateless_waveform_sample(shape, phase, 0.0) - expected).abs() < 1e-6);
            }
        }

        let pyramids = [
            (Waveform::Pyramid3, 3),
            (Waveform::Pyramid5, 5),
            (Waveform::Pyramid9, 9),
        ];
        for (shape, count) in pyramids {
            let phase = 0.173;
            let squares = count - 1;
            let expected = (0..squares)
                .map(|i| {
                    let t = (0.75 + phase + i as f32 / (2.0 * squares as f32)).rem_euclid(1.0);
                    if t < 0.5 { 1.0 } else { -1.0 }
                })
                .sum::<f32>()
                / squares as f32;
            assert!((stateless_waveform_sample(shape, phase, 0.0) - expected).abs() < 1e-6);
        }
    }

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
        let lookup = osc.sample_offset(0.0);
        assert_eq!(osc.phase(), phase_before);
        let advanced = osc.next_sample();
        assert!((lookup - advanced).abs() < f32::EPSILON);
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
        let lookup = osc.sample_offset(0.0);
        assert_eq!(lookup, 0.0);
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
        let lookup = narrow.sample_offset(0.0);
        assert_eq!(narrow.phase(), phase);
        let advanced = dry.next_sample();
        assert!((lookup - advanced).abs() < f32::EPSILON);
        let fifty_peek = fifty.sample_offset(0.0);
        assert!(
            (lookup - fifty_peek).abs() > 1e-3,
            "25% pulse must differ from 50% at t=0.35 ({lookup} vs {fifty_peek})"
        );
    }

    fn collect_n(osc: &mut Oscillator, n: usize) -> Vec<f32> {
        (0..n).map(|_| osc.next_sample()).collect()
    }

    #[test]
    fn default_white_noise_streams_are_correlated() {
        let mut a = Oscillator::from_host(Waveform::WhiteNoise, 0.1, 48_000.0);
        let mut b = Oscillator::from_host(Waveform::WhiteNoise, 0.1, 48_000.0);
        let sa = collect_n(&mut a, 64);
        let sb = collect_n(&mut b, 64);
        assert_eq!(sa, sb);
    }

    #[test]
    fn different_seeds_decorrelate_white_noise() {
        let mut a = Oscillator::from_host_with_seed(Waveform::WhiteNoise, 0.1, 48_000.0, 1);
        let mut b = Oscillator::from_host_with_seed(Waveform::WhiteNoise, 0.1, 48_000.0, 2);
        let sa = collect_n(&mut a, 64);
        let sb = collect_n(&mut b, 64);
        assert_ne!(sa, sb, "distinct seeds must produce distinct white streams");
    }

    #[test]
    fn same_seed_reproduces_white_noise() {
        let mut a = Oscillator::from_host_with_seed(Waveform::WhiteNoise, 0.1, 48_000.0, 99);
        let mut b = Oscillator::from_host_with_seed(Waveform::WhiteNoise, 0.1, 48_000.0, 99);
        assert_eq!(collect_n(&mut a, 64), collect_n(&mut b, 64));
    }

    #[test]
    fn reseed_matches_fresh_generator_from_that_seed() {
        let mut a = Oscillator::from_host_with_seed(Waveform::WhiteNoise, 0.1, 48_000.0, 1);
        let _ = collect_n(&mut a, 32);
        a.reseed(2);
        let mut b = Oscillator::from_host_with_seed(Waveform::WhiteNoise, 0.1, 48_000.0, 2);
        assert_eq!(collect_n(&mut a, 64), collect_n(&mut b, 64));
    }

    #[test]
    fn non_noise_reseed_is_noop_and_finite() {
        let mut osc = Oscillator::from_host(Waveform::Saw, 440.0, 48_000.0);
        osc.reseed(99);
        for _ in 0..64 {
            assert!(osc.next_sample().is_finite());
        }
    }

    #[test]
    fn seed_zero_matches_noise_generator_seed_one() {
        let mut via_zero = Oscillator::from_host_with_seed(Waveform::WhiteNoise, 0.1, 48_000.0, 0);
        let mut via_one = Oscillator::from_host_with_seed(Waveform::WhiteNoise, 0.1, 48_000.0, 1);
        assert_eq!(collect_n(&mut via_zero, 64), collect_n(&mut via_one, 64));

        let mut ng0 = noise::NoiseGenerator::new(noise::NoiseType::White, 0);
        let mut ng1 = noise::NoiseGenerator::new(noise::NoiseType::White, 1);
        for _ in 0..64 {
            assert!((ng0.next_sample() - ng1.next_sample()).abs() < f32::EPSILON);
        }

        let mut osc0 = Oscillator::from_host_with_seed(Waveform::WhiteNoise, 0.1, 48_000.0, 0);
        let mut ng = noise::NoiseGenerator::new(noise::NoiseType::White, 0);
        for _ in 0..64 {
            assert!((osc0.next_sample() - ng.next_sample()).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn pink_and_brown_respect_seed_and_reseed() {
        for wf in [Waveform::PinkNoise, Waveform::BrownNoise] {
            let mut same_a = Oscillator::from_host_with_seed(wf, 0.1, 48_000.0, 7);
            let mut same_b = Oscillator::from_host_with_seed(wf, 0.1, 48_000.0, 7);
            assert_eq!(collect_n(&mut same_a, 64), collect_n(&mut same_b, 64));

            let mut warmed = Oscillator::from_host_with_seed(wf, 0.1, 48_000.0, 3);
            let _ = collect_n(&mut warmed, 16);
            warmed.reseed(11);
            let mut fresh = Oscillator::from_host_with_seed(wf, 0.1, 48_000.0, 11);
            assert_eq!(collect_n(&mut warmed, 64), collect_n(&mut fresh, 64));

            let mut seed_one = Oscillator::from_host_with_seed(wf, 0.1, 48_000.0, 1);
            let mut seed_two = Oscillator::from_host_with_seed(wf, 0.1, 48_000.0, 2);
            assert_ne!(collect_n(&mut seed_one, 64), collect_n(&mut seed_two, 64));
        }
    }

    #[test]
    fn new_with_seed_matches_from_host_with_seed_on_valid_input() {
        let a = Oscillator::new_with_seed(Waveform::WhiteNoise, 0.1, 48_000.0, 5).unwrap();
        let b = Oscillator::from_host_with_seed(Waveform::WhiteNoise, 0.1, 48_000.0, 5);
        let mut a = a;
        let mut b = b;
        assert_eq!(collect_n(&mut a, 32), collect_n(&mut b, 32));
    }

    #[test]
    fn new_default_seed_matches_explicit_42() {
        let mut a = Oscillator::new(Waveform::WhiteNoise, 0.1, 48_000.0).unwrap();
        let mut b = Oscillator::new_with_seed(Waveform::WhiteNoise, 0.1, 48_000.0, 42).unwrap();
        assert_eq!(collect_n(&mut a, 32), collect_n(&mut b, 32));
    }
}
