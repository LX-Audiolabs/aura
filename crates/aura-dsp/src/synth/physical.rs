//! Physical modeling synthesis.
//!
//! Implements the Karplus-Strong plucked string algorithm, a simple
//! bidirectional waveguide for tube/string simulation, and a
//! Moog-ladder filter integrated via Runge-Kutta 4 for analog-circuit
//! accuracy. These models produce natural-sounding tones without
//! traditional oscillators or envelopes.

use serde::{Deserialize, Serialize};

use crate::delay::DelayLine;
use crate::error::Result;

/// Karplus-Strong plucked string synthesis.
///
/// A noise burst is injected into a delay line whose length determines
/// the pitch. Each pass through the loop applies a one-pole lowpass
/// (damping) filter and feedback attenuation, producing a naturally
/// decaying, pitched tone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KarplusStrong {
    /// Delay line (period = sample_rate / frequency).
    delay_line: DelayLine,
    /// Feedback coefficient (controls decay time).
    feedback: f32,
    /// One-pole lowpass filter state for damping.
    damping_prev: f32,
    /// Damping coefficient (0..1, higher = brighter).
    brightness: f32,
    /// Delay in samples (fractional for tuning accuracy).
    delay_samples: f32,
    /// Sample rate in Hz.
    sample_rate: f32,
    /// Fundamental frequency in Hz.
    frequency: f32,
    /// PRNG state for noise burst.
    noise_state: u32,
}

impl KarplusStrong {
    /// Create a new Karplus-Strong string model.
    ///
    /// # Arguments
    ///
    /// * `frequency` - Pitch in Hz
    /// * `decay` - Decay amount (0.0 = fast decay, 1.0 = long ring)
    /// * `brightness` - Damping control (0.0 = dark, 1.0 = bright)
    /// * `sample_rate` - Sample rate in Hz
    ///
    /// # Errors
    ///
    /// Returns error if frequency or sample_rate is invalid.
    pub fn new(frequency: f32, decay: f32, brightness: f32, sample_rate: f32) -> Result<Self> {
        if sample_rate <= 0.0 || !sample_rate.is_finite() {
            return Err(crate::error::NaadError::InvalidSampleRate { sample_rate });
        }
        if frequency <= 0.0 || !frequency.is_finite() || frequency >= sample_rate / 2.0 {
            return Err(crate::error::NaadError::InvalidFrequency {
                frequency,
                nyquist: sample_rate / 2.0,
            });
        }

        let delay_samples = sample_rate / frequency;
        let max_delay = (delay_samples.ceil() as usize) + 2;
        let delay_line = DelayLine::new(max_delay);

        // Feedback derived from decay: higher decay = closer to 1.0.
        let feedback = 0.9 + decay.clamp(0.0, 1.0) * 0.099; // range ~0.9..0.999

        Ok(Self {
            delay_line,
            feedback,
            damping_prev: 0.0,
            brightness: brightness.clamp(0.0, 1.0),
            delay_samples,
            sample_rate,
            frequency,
            noise_state: 54321,
        })
    }

    /// Excite the string with a noise burst (pluck).
    pub fn pluck(&mut self) {
        self.delay_line.clear();
        self.damping_prev = 0.0;
        let n = self.delay_samples.ceil() as usize;
        for _ in 0..n {
            let noise = self.next_noise();
            self.delay_line.write(noise);
        }
    }

    /// Set the fundamental frequency.
    ///
    /// # Errors
    ///
    /// Returns error if frequency is invalid.
    pub fn set_frequency(&mut self, frequency: f32) -> Result<()> {
        if frequency <= 0.0 || !frequency.is_finite() || frequency >= self.sample_rate / 2.0 {
            return Err(crate::error::NaadError::InvalidFrequency {
                frequency,
                nyquist: self.sample_rate / 2.0,
            });
        }
        self.frequency = frequency;
        self.delay_samples = self.sample_rate / frequency;
        Ok(())
    }

    /// Generate the next sample.
    #[inline]
    #[must_use]
    pub fn next_sample(&mut self) -> f32 {
        // Read from delay line at the tuned delay length.
        let delayed = self.delay_line.read(self.delay_samples);

        // One-pole lowpass for damping.
        // y[n] = brightness * x[n] + (1 - brightness) * y[n-1]
        let filtered = self.brightness * delayed + (1.0 - self.brightness) * self.damping_prev;
        self.damping_prev = crate::flush_denormal(filtered);

        // Write back with feedback.
        let feedback_sample = filtered * self.feedback;
        self.delay_line
            .write(crate::flush_denormal(feedback_sample));

        delayed
    }

    /// Fill a buffer with samples.
    #[inline]
    pub fn fill_buffer(&mut self, buffer: &mut [f32]) {
        for s in buffer.iter_mut() {
            *s = self.next_sample();
        }
    }

    /// Returns true if the string is still vibrating (output above threshold).
    ///
    /// Checks the most recent output from the delay line. Once the
    /// plucked string has decayed below the threshold, this returns false.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.damping_prev.abs() > 1e-6
    }

    /// Returns the fundamental frequency.
    #[must_use]
    pub fn frequency(&self) -> f32 {
        self.frequency
    }

    /// Signed-`f32` noise sample for the exciter.
    #[inline]
    fn next_noise(&mut self) -> f32 {
        crate::dsp_util::xorshift32_signed_f32(&mut self.noise_state)
    }
}

/// Simple bidirectional waveguide for tube/string simulation.
///
/// Two delay lines propagate waves in opposite directions with
/// reflections at the boundaries (sign inversion) and damping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Waveguide {
    /// Forward-travelling delay line.
    forward_delay: DelayLine,
    /// Backward-travelling delay line.
    backward_delay: DelayLine,
    /// Delay length in samples.
    delay_samples: f32,
    /// Damping coefficient (energy loss per round trip, 0..1).
    damping: f32,
    /// Junction reflection coefficient.
    junction_coeff: f32,
    /// Sample rate in Hz.
    sample_rate: f32,
}

impl Waveguide {
    /// Create a new bidirectional waveguide.
    ///
    /// # Arguments
    ///
    /// * `frequency` - Fundamental frequency (determines delay length)
    /// * `damping` - Energy loss per round trip (0.0 = none, 1.0 = total)
    /// * `sample_rate` - Sample rate in Hz
    ///
    /// # Errors
    ///
    /// Returns error if parameters are invalid.
    pub fn new(frequency: f32, damping: f32, sample_rate: f32) -> Result<Self> {
        if sample_rate <= 0.0 || !sample_rate.is_finite() {
            return Err(crate::error::NaadError::InvalidSampleRate { sample_rate });
        }
        if frequency <= 0.0 || !frequency.is_finite() || frequency >= sample_rate / 2.0 {
            return Err(crate::error::NaadError::InvalidFrequency {
                frequency,
                nyquist: sample_rate / 2.0,
            });
        }

        // Each delay line is half the period (wave travels in both directions).
        let half_period = sample_rate / frequency / 2.0;
        let max_delay = (half_period.ceil() as usize) + 2;

        Ok(Self {
            forward_delay: DelayLine::new(max_delay),
            backward_delay: DelayLine::new(max_delay),
            delay_samples: half_period,
            damping: damping.clamp(0.0, 1.0),
            junction_coeff: 0.99,
            sample_rate,
        })
    }

    /// Inject energy into the waveguide (excitation).
    pub fn excite(&mut self, sample: f32) {
        // Inject into both directions equally.
        let half = sample * 0.5;
        self.forward_delay.write(half);
        self.backward_delay.write(half);
    }

    /// Generate the next sample.
    ///
    /// Advances both delay lines with reflection at the boundaries.
    #[inline]
    #[must_use]
    pub fn next_sample(&mut self) -> f32 {
        // Read from both delay lines.
        let fwd = self.forward_delay.read(self.delay_samples);
        let bwd = self.backward_delay.read(self.delay_samples);

        // Reflection at boundaries: invert and apply damping.
        let attenuation = 1.0 - self.damping;
        let fwd_reflected = -bwd * attenuation * self.junction_coeff;
        let bwd_reflected = -fwd * attenuation * self.junction_coeff;

        self.forward_delay
            .write(crate::flush_denormal(fwd_reflected));
        self.backward_delay
            .write(crate::flush_denormal(bwd_reflected));

        // Output is the sum at the observation point.
        fwd + bwd
    }

    /// Fill a buffer with samples.
    #[inline]
    pub fn fill_buffer(&mut self, buffer: &mut [f32]) {
        for s in buffer.iter_mut() {
            *s = self.next_sample();
        }
    }

    /// Set the junction reflection coefficient.
    pub fn set_junction_coeff(&mut self, coeff: f32) {
        self.junction_coeff = coeff.clamp(0.0, 1.0);
    }

    /// Returns true if the waveguide still has energy above threshold.
    ///
    /// Checks the current state of both delay lines at the read position.
    #[must_use]
    pub fn is_active(&self) -> bool {
        let fwd = self.forward_delay.read(self.delay_samples);
        let bwd = self.backward_delay.read(self.delay_samples);
        (fwd + bwd).abs() > 1e-6
    }
}

/// Moog-ladder lowpass filter integrated via Runge-Kutta 4.
///
/// Models the four cascaded one-pole RC stages of the classic Moog ladder
/// circuit, with global feedback `k` for resonance and a `tanh` saturator
/// at every stage that gives the topology its characteristic warm,
/// non-linear character. The 4-state ODE system is integrated per sample
/// with one `hisab::num::rk4` step over `1 / sample_rate` seconds —
/// substantially more accurate than the trapezoidal / one-sample-Euler
/// discretisations commonly seen in DSP-equation Moog implementations,
/// especially at high resonance and near the cutoff.
///
/// State equations:
/// ```text
/// dy[0]/dt = ω_c * (tanh(input − k * y[3]) − tanh(y[0]))
/// dy[1]/dt = ω_c * (tanh(y[0]) − tanh(y[1]))
/// dy[2]/dt = ω_c * (tanh(y[1]) − tanh(y[2]))
/// dy[3]/dt = ω_c * (tanh(y[2]) − tanh(y[3]))
/// ```
/// where `ω_c = 2π · cutoff_hz` and `k ∈ [0, 4]` (k ≈ 4 → self-oscillation).
///
/// Requires the `synthesis` feature (uses hisab ODE solver).
#[cfg(feature = "synthesis")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoogLadder {
    /// Cutoff frequency in Hz.
    cutoff_hz: f32,
    /// Resonance feedback amount (0.0 = no resonance, ~4.0 = self-oscillation).
    resonance: f32,
    /// Sample rate in Hz.
    sample_rate: f32,
    /// Four ladder-stage states (f64 for ODE-integrator stability).
    state: [f64; 4],
}

#[cfg(feature = "synthesis")]
impl MoogLadder {
    /// Create a new Moog-ladder filter.
    ///
    /// # Errors
    ///
    /// Returns [`crate::NaadError::InvalidSampleRate`] if `sample_rate <= 0`,
    /// or [`crate::NaadError::InvalidFrequency`] if `cutoff_hz` is non-positive
    /// or above Nyquist.
    pub fn new(cutoff_hz: f32, resonance: f32, sample_rate: f32) -> Result<Self> {
        if let Some(e) = crate::error::validate_sample_rate(sample_rate) {
            return Err(e);
        }
        if let Some(e) = crate::error::validate_frequency(cutoff_hz, sample_rate) {
            return Err(e);
        }
        Ok(Self {
            cutoff_hz,
            resonance: resonance.clamp(0.0, 4.0),
            sample_rate,
            state: [0.0; 4],
        })
    }

    /// Set the cutoff frequency in Hz.
    ///
    /// # Errors
    ///
    /// Returns [`crate::NaadError::InvalidFrequency`] if out of valid range.
    pub fn set_cutoff(&mut self, cutoff_hz: f32) -> Result<()> {
        if let Some(e) = crate::error::validate_frequency(cutoff_hz, self.sample_rate) {
            return Err(e);
        }
        self.cutoff_hz = cutoff_hz;
        Ok(())
    }

    /// Set the resonance amount. Clamped to `[0.0, 4.0]`.
    pub fn set_resonance(&mut self, resonance: f32) {
        self.resonance = resonance.clamp(0.0, 4.0);
    }

    /// Returns the current cutoff in Hz.
    #[inline]
    #[must_use]
    pub fn cutoff_hz(&self) -> f32 {
        self.cutoff_hz
    }

    /// Returns the current resonance amount.
    #[inline]
    #[must_use]
    pub fn resonance(&self) -> f32 {
        self.resonance
    }

    /// Reset internal state to zero.
    pub fn reset(&mut self) {
        self.state = [0.0; 4];
    }

    /// Process a single sample through the Moog ladder.
    ///
    /// Takes one RK4 step from `t = 0` to `t = 1/sample_rate` over the
    /// 4-state system, then returns the lowpass-most stage `y[3]`.
    #[inline]
    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let omega = 2.0 * std::f64::consts::PI * self.cutoff_hz as f64;
        let k = self.resonance as f64;
        let dt = 1.0 / self.sample_rate as f64;
        let input_f64 = input as f64;

        let derivative = |_t: f64, y: &[f64], dy: &mut [f64]| {
            let stage_in = (input_f64 - k * y[3]).tanh();
            let t0 = y[0].tanh();
            let t1 = y[1].tanh();
            let t2 = y[2].tanh();
            let t3 = y[3].tanh();
            dy[0] = omega * (stage_in - t0);
            dy[1] = omega * (t0 - t1);
            dy[2] = omega * (t1 - t2);
            dy[3] = omega * (t2 - t3);
        };

        if let Ok(new_state) = hisab::num::rk4(derivative, 0.0, &self.state, dt, 1)
            && new_state.len() == 4
        {
            self.state[0] = crate::flush_denormal(new_state[0] as f32) as f64;
            self.state[1] = crate::flush_denormal(new_state[1] as f32) as f64;
            self.state[2] = crate::flush_denormal(new_state[2] as f32) as f64;
            self.state[3] = crate::flush_denormal(new_state[3] as f32) as f64;
        }
        // Defensive guard against the rare case where the integrator
        // diverges (e.g., extreme cutoff/resonance combinations) — clamp
        // the output to a sane audio range rather than emitting garbage.
        (self.state[3] as f32).clamp(-2.0, 2.0)
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
    fn test_ks_produces_pitched_output() {
        let mut ks = KarplusStrong::new(440.0, 0.9, 0.5, 44100.0).unwrap();
        ks.pluck();

        let mut buf = [0.0f32; 2048];
        ks.fill_buffer(&mut buf);
        assert!(
            buf.iter().any(|&s| s.abs() > 0.1),
            "KS should produce output after pluck"
        );
        assert!(buf.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn test_ks_decays() {
        let mut ks = KarplusStrong::new(440.0, 0.0, 0.5, 44100.0).unwrap();
        ks.pluck();

        // Run for a while — with low decay it should die out.
        let mut buf = [0.0f32; 512];
        for _ in 0..100 {
            ks.fill_buffer(&mut buf);
        }
        let peak = buf.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(
            peak < 0.1,
            "KS with low decay should attenuate, peak={peak}"
        );
    }

    #[test]
    fn test_ks_frequency_matches() {
        // Use a low frequency with low brightness to get a clean
        // fundamental for zero-crossing detection.
        let freq = 200.0;
        let sr = 44100.0;
        let mut ks = KarplusStrong::new(freq, 0.95, 0.3, sr).unwrap();
        ks.pluck();

        // The expected period in samples.
        let expected_period = sr / freq; // ~220.5

        // Skip initial transient to let waveform settle.
        let mut skip_buf = vec![0.0f32; 4096];
        ks.fill_buffer(&mut skip_buf);

        // Collect samples after transient.
        let num_samples = 16384;
        let mut samples = vec![0.0f32; num_samples];
        ks.fill_buffer(&mut samples);

        // Use autocorrelation to estimate the period — more robust than
        // zero-crossing detection for signals with harmonics.
        let search_min = (expected_period * 0.8) as usize;
        let search_max = (expected_period * 1.2) as usize;
        let window = 4096;

        let mut best_lag = search_min;
        let mut best_corr = f32::NEG_INFINITY;
        for lag in search_min..=search_max.min(window) {
            let mut corr = 0.0f32;
            for i in 0..(window - lag) {
                corr += samples[i] * samples[i + lag];
            }
            if corr > best_corr {
                best_corr = corr;
                best_lag = lag;
            }
        }

        let error = (best_lag as f32 - expected_period).abs() / expected_period;
        assert!(
            error < 0.05,
            "frequency should be close to expected: measured_period={best_lag}, expected={expected_period}"
        );
    }

    #[test]
    fn test_waveguide_produces_output() {
        let mut wg = Waveguide::new(220.0, 0.01, 44100.0).unwrap();

        // Excite with a short burst.
        for _ in 0..10 {
            wg.excite(1.0);
            let _ = wg.next_sample();
        }

        let mut buf = [0.0f32; 1024];
        wg.fill_buffer(&mut buf);
        assert!(
            buf.iter().any(|&s| s.abs() > 0.001),
            "waveguide should produce output after excitation"
        );
        assert!(buf.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn test_ks_serde_roundtrip() {
        let ks = KarplusStrong::new(440.0, 0.9, 0.5, 44100.0).unwrap();
        let json = serde_json::to_string(&ks).unwrap();
        let back: KarplusStrong = serde_json::from_str(&json).unwrap();
        assert!((ks.frequency - back.frequency).abs() < f32::EPSILON);
        assert!((ks.feedback - back.feedback).abs() < f32::EPSILON);
    }

    #[test]
    fn test_waveguide_serde_roundtrip() {
        let wg = Waveguide::new(220.0, 0.01, 44100.0).unwrap();
        let json = serde_json::to_string(&wg).unwrap();
        let back: Waveguide = serde_json::from_str(&json).unwrap();
        assert!((wg.damping - back.damping).abs() < f32::EPSILON);
        assert!((wg.delay_samples - back.delay_samples).abs() < f32::EPSILON);
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_moog_ladder_attenuates_high_freq() {
        // 1 kHz cutoff: a 5 kHz sine should be heavily attenuated; a
        // 200 Hz sine should pass through near-unity (post-warmup).
        let sr = 44100.0;
        let mut filter = MoogLadder::new(1000.0, 0.0, sr).unwrap();

        let measure_rms = |filter: &mut MoogLadder, freq: f32| -> f32 {
            filter.reset();
            let n = 8192usize;
            // Warmup
            for i in 0..1024 {
                let t = i as f32 / sr;
                let _ = filter.process_sample((t * freq * std::f32::consts::TAU).sin());
            }
            // Measure
            let mut sum_sq = 0.0f32;
            for i in 1024..(1024 + n) {
                let t = i as f32 / sr;
                let s = filter.process_sample((t * freq * std::f32::consts::TAU).sin());
                sum_sq += s * s;
            }
            (sum_sq / n as f32).sqrt()
        };

        let rms_low = measure_rms(&mut filter, 200.0);
        let rms_high = measure_rms(&mut filter, 5000.0);

        assert!(rms_low > 0.4, "200 Hz should pass; RMS = {rms_low}");
        assert!(
            rms_high < rms_low * 0.5,
            "5 kHz should be attenuated below half of 200 Hz; \
             rms_low = {rms_low}, rms_high = {rms_high}"
        );
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_moog_ladder_resonance_rings() {
        // High resonance + an impulse → output should ring (decaying
        // oscillation around the cutoff). Compare resonance=0 (no ring)
        // vs resonance=3.9 (strong ring): the resonant version's tail
        // energy should clearly exceed the non-resonant baseline.
        let measure_tail_rms = |res: f32| -> f32 {
            let mut filter = MoogLadder::new(800.0, res, 44100.0).unwrap();
            let mut buf = vec![0.0f32; 4096];
            buf[0] = 1.0;
            filter.process_buffer(&mut buf);
            (buf[200..1200].iter().map(|s| s * s).sum::<f32>() / 1000.0).sqrt()
        };

        let tail_dry = measure_tail_rms(0.0);
        let tail_resonant = measure_tail_rms(3.9);

        assert!(
            tail_resonant > tail_dry * 4.0 && tail_resonant > 1e-4,
            "resonant tail should dominate the dry tail: dry={tail_dry}, resonant={tail_resonant}"
        );
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_moog_ladder_stable_under_dc() {
        // DC input + extreme parameters: filter must not blow up.
        let mut filter = MoogLadder::new(2000.0, 4.0, 44100.0).unwrap();
        let mut buf = vec![0.5f32; 8192];
        filter.process_buffer(&mut buf);
        assert!(buf.iter().all(|s| s.is_finite() && s.abs() < 2.0));
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_moog_ladder_invalid_inputs() {
        assert!(MoogLadder::new(1000.0, 1.0, 0.0).is_err());
        assert!(MoogLadder::new(-100.0, 1.0, 44100.0).is_err());
        assert!(MoogLadder::new(50000.0, 1.0, 44100.0).is_err());
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_moog_ladder_resonance_clamps() {
        // Out-of-range resonance should clamp into [0, 4] without panicking.
        let mut filter = MoogLadder::new(1000.0, 100.0, 44100.0).unwrap();
        assert!((filter.resonance() - 4.0).abs() < f32::EPSILON);
        filter.set_resonance(-1.0);
        assert!((filter.resonance() - 0.0).abs() < f32::EPSILON);
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_moog_ladder_serde_roundtrip() {
        let mut filter = MoogLadder::new(1500.0, 1.5, 48000.0).unwrap();
        // Drive some state in
        for i in 0..256 {
            let _ = filter.process_sample(((i as f32) * 0.01).sin());
        }
        let json = serde_json::to_string(&filter).unwrap();
        let back: MoogLadder = serde_json::from_str(&json).unwrap();
        assert!((filter.cutoff_hz() - back.cutoff_hz()).abs() < f32::EPSILON);
        assert!((filter.resonance() - back.resonance()).abs() < f32::EPSILON);
    }
}
