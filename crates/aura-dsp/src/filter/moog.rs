//! Moog ladder filter — 4th-order resonant lowpass.
//!
//! Ported from fundsp `src/moog.rs` (MIT / Apache-2.0).
//! Original authors: Sami Perttu and contributors.
//!
//! Nonlinear 4-stage cascade with feedback for the classic warm Moog LPF.
//! For a different ladder topology (ZDF + linear prediction), see
//! [`super::PredictiveLadder`].

use crate::error::{DspError, Result};

/// Moog ladder lowpass filter — 4th-order with resonant feedback.
///
/// Uses a 4-stage cascade of one-pole filters with a `tanh` saturator
/// on the final stage. Feedback through the resonance path (`rez`)
/// creates the characteristic Moog emphasis near cutoff.
///
/// # Examples
///
/// ```rust
/// use aura_dsp::filter::MoogLadder;
/// let mut moog = MoogLadder::new(44100.0, 1000.0, 0.4).unwrap();
/// let output = moog.process_sample(0.5);
/// ```
#[derive(Debug, Clone)]
pub struct MoogLadder {
    sample_rate: f32,
    cutoff: f32,
    q: f32,
    // Derived coefficients
    p: f32,
    k: f32,
    rez: f32,
    // 4-pole state
    s0: f32,
    s1: f32,
    s2: f32,
    s3: f32,
    // Previous values (trapezoidal integration)
    px: f32,
    ps0: f32,
    ps1: f32,
    ps2: f32,
}

impl MoogLadder {
    /// Create a new Moog ladder filter.
    ///
    /// # Errors
    ///
    /// Returns [`DspError::InvalidSampleRate`] if `sample_rate <= 0`.
    /// Returns [`DspError::InvalidFrequency`] if `cutoff` is out of valid range.
    pub fn new(sample_rate: f32, cutoff: f32, q: f32) -> Result<Self> {
        if sample_rate <= 0.0 {
            return Err(DspError::InvalidSampleRate { sample_rate });
        }
        let nyquist = sample_rate / 2.0;
        if cutoff <= 0.0 || cutoff >= nyquist || !cutoff.is_finite() {
            return Err(DspError::InvalidFrequency {
                frequency: cutoff,
                nyquist,
            });
        }
        let mut filter = Self {
            sample_rate,
            cutoff: 0.0,
            q: 0.0,
            p: 0.0,
            k: 0.0,
            rez: 0.0,
            s0: 0.0,
            s1: 0.0,
            s2: 0.0,
            s3: 0.0,
            px: 0.0,
            ps0: 0.0,
            ps1: 0.0,
            ps2: 0.0,
        };
        filter.set_cutoff_q(cutoff, q);
        Ok(filter)
    }

    /// Set cutoff frequency (Hz) and resonance (Q).
    ///
    /// Q values above ~0.5 produce self-oscillation.
    /// Typical musical range: 0.0–0.9.
    #[inline]
    pub fn set_cutoff_q(&mut self, cutoff: f32, q: f32) {
        self.cutoff = cutoff;
        self.q = q;
        let c = 2.0 * cutoff / self.sample_rate;
        self.p = c * (1.8 - 0.8 * c);
        self.k = 2.0 * (c * std::f32::consts::PI * 0.5).sin() - 1.0;
        let t1 = (1.0 - self.p) * 1.386_249;
        let t2 = 12.0 + t1 * t1;
        self.rez = q * (t2 + 6.0 * t1) / (t2 - 6.0 * t1);
    }

    /// Set cutoff frequency (Hz).
    #[inline]
    pub fn set_cutoff(&mut self, cutoff: f32) {
        self.set_cutoff_q(cutoff, self.q);
    }

    /// Set resonance (Q).
    #[inline]
    pub fn set_q(&mut self, q: f32) {
        self.set_cutoff_q(self.cutoff, q);
    }

    /// Current cutoff frequency (Hz).
    #[inline]
    pub fn cutoff(&self) -> f32 {
        self.cutoff
    }

    /// Current resonance (Q).
    #[inline]
    pub fn q(&self) -> f32 {
        self.q
    }

    /// Reset filter state to zero.
    #[inline]
    pub fn reset(&mut self) {
        self.s0 = 0.0;
        self.s1 = 0.0;
        self.s2 = 0.0;
        self.s3 = 0.0;
        self.px = 0.0;
        self.ps0 = 0.0;
        self.ps1 = 0.0;
        self.ps2 = 0.0;
    }

    /// Process a single sample through the filter.
    #[inline]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        // Feedback through resonance
        let x = -self.rez * self.s3 + input;

        // 4-stage cascade with trapezoidal integration
        // Stage 0
        self.s0 = (x + self.px) * self.p - self.k * self.s0;
        // Stage 1
        self.s1 = (self.s0 + self.ps0) * self.p - self.k * self.s1;
        // Stage 2
        self.s2 = (self.s1 + self.ps1) * self.p - self.k * self.s2;
        // Stage 3 with tanh saturation
        self.s3 = tanh_approx((self.s2 + self.ps2) * self.p - self.k * self.s3);

        // Save state for next sample
        self.px = x;
        self.ps0 = self.s0;
        self.ps1 = self.s1;
        self.ps2 = self.s2;

        self.s3
    }

    /// Process a buffer of samples in-place.
    #[inline]
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process_sample(*sample);
        }
    }
}

/// Fast tanh approximation for the Moog saturation stage.
///
/// Uses a rational approximation that is faster than `f32::tanh`
/// while preserving the musical saturation character.
#[inline]
fn tanh_approx(x: f32) -> f32 {
    let x2 = x * x;
    let numerator = x * (27.0 + x2);
    let denominator = 27.0 + 9.0 * x2;
    numerator / denominator
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moog_new_valid() {
        let m = MoogLadder::new(44100.0, 1000.0, 0.4);
        assert!(m.is_ok());
    }

    #[test]
    fn moog_new_invalid_sr() {
        let m = MoogLadder::new(0.0, 1000.0, 0.4);
        assert!(m.is_err());
    }

    #[test]
    fn moog_new_invalid_freq() {
        let m = MoogLadder::new(44100.0, 30000.0, 0.4);
        assert!(m.is_err());
    }

    #[test]
    fn moog_passes_signal() {
        let mut m = MoogLadder::new(44100.0, 10000.0, 0.0).unwrap();
        // Moog ladder with zero resonance passes signal (with slight attenuation)
        for _ in 0..100 {
            m.process_sample(1.0);
        }
        let settled = m.process_sample(1.0);
        // Should pass most of the DC signal (>0.5)
        assert!(settled > 0.5 && settled.is_finite());
    }

    #[test]
    fn moog_resonance_does_not_explode() {
        let mut m = MoogLadder::new(44100.0, 1000.0, 0.8).unwrap();
        for _ in 0..1000 {
            let out = m.process_sample(0.1);
            assert!(out.is_finite(), "output should be finite");
        }
    }

    #[test]
    fn moog_reset_clears_state() {
        let mut m = MoogLadder::new(44100.0, 1000.0, 0.4).unwrap();
        for _ in 0..100 {
            m.process_sample(1.0);
        }
        m.reset();
        let out = m.process_sample(0.0);
        assert!(out.abs() < 0.001);
    }
}
