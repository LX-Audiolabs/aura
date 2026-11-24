//! Predictive ladder filter — ZDF Moog ladder with linear prediction.
//!
//! Ported from infinitedsp `src/effects/filter/predictive_ladder.rs` (MIT).
//! Original author: Na1w (github.com/Na1w/infinitedsp).
//!
//! The predictive ladder uses Zero-Delay Feedback (ZDF) with a linear
//! prediction step instead of the Newton-Raphson iterative solver used
//! by the standard [`LadderFilter`]. This makes it significantly faster
//! while retaining comparable audio quality.
//!
//! Compared to [`super::moog::MoogLadder`]:
//! - Better tuning accuracy at high resonance (trapezoidal ZDF)
//! - Linear predictor instead of heuristic feedback
//! - Uses `tanh` saturation, same rational approximation

use std::f32::consts::PI;

use crate::error::{DspError, Result};

/// 4-pole lowpass ladder filter using Linear Prediction ZDF.
///
/// State is stored as 4 trapezoidal integrator outputs `s[0..3]`.
/// At each step:
/// 1. Predict the output using `y_est = (gamma*x + sigma) / (1 + k*gamma)`
/// 2. Compute feedback `u = x - k*tanh(y_est)`
/// 3. Run 4 stages forward, update integrator state
#[derive(Debug, Clone)]
pub struct PredictiveLadder {
    sample_rate: f32,
    cutoff: f32,
    q: f32,
    // Cached coefficients
    g: f32,
    k: f32,
    beta: f32,
    // 4-pole trapezoidal state
    s: [f32; 4],
}

impl PredictiveLadder {
    /// Create a new predictive ladder filter.
    ///
    /// `resonance` in 0..1+; self-oscillates at high values (~0.9+).
    ///
    /// # Errors
    ///
    /// Returns [`DspError::InvalidSampleRate`] if `sample_rate <= 0`.
    /// Returns [`DspError::InvalidFrequency`] if `cutoff` is out of range.
    pub fn new(sample_rate: f32, cutoff: f32, q: f32) -> Result<Self> {
        if sample_rate <= 0.0 || !sample_rate.is_finite() {
            return Err(DspError::InvalidSampleRate { sample_rate });
        }
        let nyquist = sample_rate / 2.0;
        if cutoff <= 0.0 || cutoff >= nyquist || !cutoff.is_finite() {
            return Err(DspError::InvalidFrequency {
                frequency: cutoff,
                nyquist,
            });
        }
        if !q.is_finite() || q < 0.0 || q > f32::MAX / 4.0 {
            return Err(DspError::InvalidFilterParameters);
        }
        let mut filter = Self {
            sample_rate,
            cutoff: 0.0,
            q: 0.0,
            g: 0.0,
            k: 0.0,
            beta: 0.0,
            s: [0.0; 4],
        };
        filter.set_cutoff_q(cutoff, q);
        Ok(filter)
    }

    /// Set cutoff frequency (Hz) and resonance. Invalid values leave parameters unchanged.
    #[inline]
    pub fn set_cutoff_q(&mut self, cutoff: f32, q: f32) {
        if !cutoff.is_finite()
            || cutoff <= 0.0
            || cutoff >= self.sample_rate * 0.5
            || !q.is_finite()
            || q < 0.0
            || q > f32::MAX / 4.0
        {
            return;
        }
        self.cutoff = cutoff;
        self.q = q;
        let max_f = self.sample_rate * 0.49;
        let fc = cutoff.min(max_f);
        self.g = (PI * fc / self.sample_rate).tan();
        self.k = self.q * 4.0;
        self.beta = 1.0 / (1.0 + self.g);
    }

    /// Set cutoff frequency (Hz).
    #[inline]
    pub fn set_cutoff(&mut self, cutoff: f32) {
        self.set_cutoff_q(cutoff, self.q);
    }

    /// Set resonance.
    #[inline]
    pub fn set_q(&mut self, q: f32) {
        self.set_cutoff_q(self.cutoff, q);
    }

    /// Current cutoff frequency (Hz).
    #[inline]
    pub fn cutoff(&self) -> f32 {
        self.cutoff
    }

    /// Current resonance.
    #[inline]
    pub fn q(&self) -> f32 {
        self.q
    }

    /// Reset filter state to zero.
    #[inline]
    pub fn reset(&mut self) {
        self.s = [0.0; 4];
    }

    /// Process a single sample.
    #[inline]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let mut sample = input;
        let (g, k, beta) = (self.g, self.k, self.beta);
        Self::step(&mut self.s, &mut sample, g, k, beta);
        sample
    }

    /// Process a buffer in-place.
    #[inline]
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        let (g, k, beta) = (self.g, self.k, self.beta);
        for sample in buffer.iter_mut() {
            Self::step(&mut self.s, sample, g, k, beta);
        }
    }

    /// Single step of the predictive ladder.
    ///
    /// Algorithm:
    /// 1. Compute `g_val = g * beta`, `gamma = (g_val)^4`
    /// 2. Forward-predict sigma: `sigma = s3 + g_val*(s2 + g_val*(s1 + g_val*s0))`
    /// 3. Estimate output: `y_est = (gamma*x + sigma) / (1 + k*gamma)`
    /// 4. Feedback: `u = x - k*tanh(y_est)`
    /// 5. Run 4 trapezoidal stages forward, update integrators
    #[inline]
    #[allow(clippy::many_single_char_names)]
    fn step(s: &mut [f32; 4], sample: &mut f32, g: f32, k: f32, beta: f32) {
        if !sample.is_finite() {
            *sample = 0.0;
            return;
        }
        let x = *sample;

        let g_val = g * beta;
        let s0 = s[0] * beta;
        let s1 = s[1] * beta;
        let s2 = s[2] * beta;
        let s3 = s[3] * beta;

        let g2 = g_val * g_val;
        let gamma = g2 * g2;

        // Forward prediction
        let sigma = s3 + g_val * (s2 + g_val * (s1 + g_val * s0));

        // Predict y4 using linear approximation
        let y_est = (gamma * x + sigma) / (1.0 + k * gamma);

        // Feedback with predicted y4
        let u = x - k * fast_tanh(y_est);

        // Run stages forward
        let v1 = g_val * u + s0;
        let v2 = g_val * v1 + s1;
        let v3 = g_val * v2 + s2;
        let v4 = g_val * v3 + s3;

        // Update state (trapezoidal integrator update: s[n] = 2*v - s[n-1])
        s[0] = crate::flush_denormal(2.0 * v1 - s[0]);
        s[1] = crate::flush_denormal(2.0 * v2 - s[1]);
        s[2] = crate::flush_denormal(2.0 * v3 - s[2]);
        s[3] = crate::flush_denormal(2.0 * v4 - s[3]);

        if !v4.is_finite() || s.iter().any(|x| !x.is_finite()) {
            *s = [0.0; 4];
            *sample = 0.0;
        } else {
            *sample = v4;
        }
    }
}

/// Fast tanh(x) — rational approximation.
///
/// `tanh(x) ≈ x*(27 + x²) / (27 + 9*x²)`, clamped to [-3, 3] for stability.
/// This matches the infinitedsp implementation exactly.
#[inline]
fn fast_tanh(x: f32) -> f32 {
    let x_clamped = x.clamp(-3.0, 3.0);
    let x2 = x_clamped * x_clamped;
    x_clamped * (27.0 + x2) / (27.0 + 9.0 * x2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predictive_ladder_new_valid() {
        assert!(PredictiveLadder::new(44100.0, 1000.0, 0.5).is_ok());
    }

    #[test]
    fn predictive_ladder_invalid_sr() {
        assert!(PredictiveLadder::new(0.0, 1000.0, 0.5).is_err());
    }

    #[test]
    fn predictive_ladder_invalid_freq() {
        assert!(PredictiveLadder::new(44100.0, 30000.0, 0.5).is_err());
    }

    #[test]
    fn predictive_ladder_passes_dc() {
        let mut f = PredictiveLadder::new(44100.0, 20000.0, 0.0).unwrap();
        for _ in 0..200 {
            f.process_sample(1.0);
        }
        let out = f.process_sample(1.0);
        assert!((out - 1.0).abs() < 0.1);
    }

    #[test]
    fn predictive_ladder_resonance_stable() {
        let mut f = PredictiveLadder::new(44100.0, 1000.0, 0.9).unwrap();
        for _ in 0..2000 {
            let out = f.process_sample(0.1);
            assert!(out.is_finite());
        }
    }

    #[test]
    fn predictive_ladder_reset_clears_state() {
        let mut f = PredictiveLadder::new(44100.0, 1000.0, 0.5).unwrap();
        for _ in 0..100 {
            f.process_sample(1.0);
        }
        f.reset();
        let out = f.process_sample(0.0);
        assert!(out.abs() < 0.001);
    }
}

#[cfg(test)]
mod regressions {
    use super::*;
    #[test]
    fn predictive_prewarp_tracks_high_cutoff() {
        let f = PredictiveLadder::new(48000.0, 20000.0, 0.0).unwrap();
        let effective = f.g.atan() * 48000.0 / PI;
        assert!((effective - 20000.0).abs() < 1.0, "{effective}");
    }
    #[test]
    fn predictive_rejects_bad_parameters_and_recovers_bad_input() {
        assert!(PredictiveLadder::new(f32::NAN, 1000.0, 0.5).is_err());
        assert!(PredictiveLadder::new(48000.0, 1000.0, f32::NAN).is_err());
        let mut f = PredictiveLadder::new(48000.0, 1000.0, 0.5).unwrap();
        f.set_cutoff_q(f32::NAN, 1.0);
        assert_eq!(f.cutoff(), 1000.0);
        f.set_cutoff_q(2000.0, f32::MAX);
        assert_eq!(f.cutoff(), 1000.0);
        assert_eq!(f.process_sample(f32::NAN), 0.0);
        assert!(f.process_sample(0.1).is_finite());
    }
}
