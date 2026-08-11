//! Physical modeling synthesis.
//!
//! Karplus–Strong plucked string and a simple bidirectional waveguide
//! for tube/string simulation. Ladder filters live under [`crate::filter`]
//! (`MoogLadder`, `PredictiveLadder`) — not duplicated here.

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
            return Err(crate::error::DspError::InvalidSampleRate { sample_rate });
        }
        if frequency <= 0.0 || !frequency.is_finite() || frequency >= sample_rate / 2.0 {
            return Err(crate::error::DspError::InvalidFrequency {
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
            return Err(crate::error::DspError::InvalidFrequency {
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
            return Err(crate::error::DspError::InvalidSampleRate { sample_rate });
        }
        if frequency <= 0.0 || !frequency.is_finite() || frequency >= sample_rate / 2.0 {
            return Err(crate::error::DspError::InvalidFrequency {
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

}
