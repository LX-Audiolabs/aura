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
/// on the final stage, antialiased with first-order ADAA of the existing
/// rational transfer curve. Four cascade stages and their coefficients are retained;
/// ADAA adds temporal smoothing and changes the final stage feedback dynamics.
/// Feedback through the resonance path (`rez`)
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
    saturation_previous: f32,
}

impl MoogLadder {
    /// Retarget coefficients without resetting signal history. Unchanged rate is a no-op.
    ///
    /// # Errors
    /// Rejects invalid rates or a current cutoff at/above the new Nyquist;
    /// errors leave parameters and state unchanged. Allocates nothing.
    pub fn set_sample_rate(&mut self, sample_rate: f32) -> Result<()> {
        if let Some(err) = crate::error::validate_sample_rate(sample_rate) {
            return Err(err);
        }
        if sample_rate == self.sample_rate {
            return Ok(());
        }
        if let Some(err) = crate::error::validate_frequency(self.cutoff, sample_rate) {
            return Err(err);
        }
        self.sample_rate = sample_rate;
        self.set_cutoff_q(self.cutoff, self.q);
        Ok(())
    }

    /// Create a new Moog ladder filter.
    ///
    /// # Errors
    ///
    /// Returns [`DspError::InvalidSampleRate`] if `sample_rate <= 0`.
    /// Returns [`DspError::InvalidFrequency`] if `cutoff` is out of valid range.
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
            saturation_previous: 0.0,
        };
        filter.set_cutoff_q(cutoff, q);
        Ok(filter)
    }

    /// Set cutoff frequency (Hz) and resonance (Q). Invalid values leave parameters unchanged.
    ///
    /// Q values above ~0.5 produce self-oscillation.
    /// Typical musical range: 0.0–0.9.
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
        let c = 2.0 * cutoff / self.sample_rate;
        self.p = c * (1.8 - 0.8 * c);
        self.k = 2.0 * (c * std::f32::consts::PI * 0.5).sin() - 1.0;
        let t1 = (1.0 - self.p) * 1.386_249;
        let t2 = 12.0 + t1 * t1;
        self.rez = self.q * ((t2 + 6.0 * t1) / (t2 - 6.0 * t1));
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
        self.saturation_previous = 0.0;
    }

    /// Process a single sample through the filter.
    #[inline]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        if !input.is_finite() {
            return 0.0;
        }
        // Feedback through resonance
        let x = crate::flush_denormal(-self.rez * self.s3 + input);

        // 4-stage cascade with trapezoidal integration
        // Stage 0
        self.s0 = crate::flush_denormal((x + self.px) * self.p - self.k * self.s0);
        // Stage 1
        self.s1 = crate::flush_denormal((self.s0 + self.ps0) * self.p - self.k * self.s1);
        // Stage 2
        self.s2 = crate::flush_denormal((self.s1 + self.ps1) * self.p - self.k * self.s2);
        // Stage 3 with tanh saturation
        self.s3 = crate::flush_denormal(saturate_adaa(
            (self.s2 + self.ps2) * self.p - self.k * self.s3,
            &mut self.saturation_previous,
        ));

        // Save state for next sample
        self.px = x;
        self.ps0 = self.s0;
        self.ps1 = self.s1;
        self.ps2 = self.s2;

        if !self.s0.is_finite()
            || !self.s1.is_finite()
            || !self.s2.is_finite()
            || !self.s3.is_finite()
        {
            self.reset();
            return 0.0;
        }
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
    let x = x.clamp(-3.0, 3.0);
    let x2 = x * x;
    let numerator = x * (27.0 + x2);
    let denominator = 27.0 + 9.0 * x2;
    numerator / denominator
}

// Exact primitive of the existing clamped rational tanh approximation.
// Using log(cosh) here would subtly replace the ladder's static transfer curve.
fn saturation_primitive(x: f64) -> f64 {
    let a = x.abs().min(3.0);
    a * a / 18.0 + (4.0 / 3.0) * (a * a / 3.0).ln_1p() + (x.abs() - 3.0).max(0.0)
}

#[inline]
fn saturate_adaa(x: f32, previous: &mut f32) -> f32 {
    if !x.is_finite() {
        *previous = 0.0;
        return 0.0;
    }
    let a = f64::from(x);
    let b = f64::from(*previous);
    *previous = crate::flush_denormal(x);
    if (a - b).abs() < 1e-5 * (1.0 + a.abs().max(b.abs())) {
        tanh_approx((0.5 * a + 0.5 * b) as f32)
    } else {
        ((saturation_primitive(a) - saturation_primitive(b)) / (a - b)) as f32
    }
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

#[cfg(test)]
mod regressions {
    use super::*;
    #[test]
    fn moog_rejects_invalid_parameters_and_recovers_bad_input() {
        assert!(MoogLadder::new(f32::NAN, 1000.0, 0.5).is_err());
        assert!(MoogLadder::new(48000.0, 1000.0, f32::NAN).is_err());
        let mut f = MoogLadder::new(48000.0, 1000.0, 0.5).unwrap();
        f.set_cutoff_q(f32::NAN, 1.0);
        assert_eq!(f.cutoff(), 1000.0);
        f.set_cutoff_q(2000.0, f32::MAX);
        assert_eq!(f.cutoff(), 1000.0);
        assert_eq!(f.process_sample(f32::NAN), 0.0);
        assert!(f.process_sample(0.1).is_finite());
    }
    #[test]
    fn moog_saturator_stays_bounded() {
        for x in [-100.0, -10.0, 10.0, 100.0] {
            assert!(tanh_approx(x).abs() <= 1.0);
        }
    }
}

#[cfg(test)]
mod adaa_tests {
    use super::*;
    // Reference is the pre-ADAA ladder recurrence, with the same poles and feedback.
    fn naive_step(f: &mut MoogLadder, input: f32) -> f32 {
        let x = input - f.rez * f.s3;
        f.s0 = (x + f.px) * f.p - f.k * f.s0;
        f.s1 = (f.s0 + f.ps0) * f.p - f.k * f.s1;
        f.s2 = (f.s1 + f.ps1) * f.p - f.k * f.s2;
        f.s3 = tanh_approx((f.s2 + f.ps2) * f.p - f.k * f.s3);
        f.px = x;
        f.ps0 = f.s0;
        f.ps1 = f.s1;
        f.ps2 = f.s2;
        f.s3
    }

    #[test]
    fn high_cutoff_driven_ladder_alias_energy_reduced() {
        let mut filter = MoogLadder::new(48000.0, 18000.0, 0.2).unwrap();
        let mut reference = filter.clone();
        let mut clean = vec![];
        let mut naive = vec![];
        for n in 0..16384 {
            let phase = std::f64::consts::TAU * 1703.0 * f64::from(n) / 8192.0;
            let input = (8.0 * phase.sin()) as f32;
            let y = filter.process_sample(input);
            let r = naive_step(&mut reference, input);
            if n >= 8192 {
                clean.push(f64::from(y));
                naive.push(f64::from(r));
            }
        }
        let fft = realfft::RealFftPlanner::<f64>::new().plan_fft_forward(8192);
        let energy = |samples: &mut [f64]| {
            let mut spectrum = fft.make_output_vec();
            fft.process(samples, &mut spectrum).unwrap();
            let fundamental = spectrum[1703].norm_sqr();
            spectrum
                .iter()
                .enumerate()
                .skip(1)
                .filter(|(i, _)| *i != 1703 && *i != 3406)
                .map(|(_, z)| z.norm_sqr())
                .sum::<f64>()
                / fundamental
        };
        let a = energy(&mut clean);
        let b = energy(&mut naive);
        assert!(a < b * 0.5, "alias energy ADAA={a} naive={b}");
    }

    #[test]
    fn adaa_preserves_static_transfer_and_handles_small_differences() {
        for x in [-100.0, -3.0, -1.0, -0.00001, 0.0, 0.00001, 1.0, 3.0, 100.0] {
            let mut previous = x;
            assert_eq!(saturate_adaa(x, &mut previous), tanh_approx(x));
            let y = saturate_adaa(x + 1e-6, &mut previous);
            assert!((y - tanh_approx(x)).abs() < 1e-5);
        }
    }

    #[test]
    fn high_resonance_cutoff_drive_stay_finite() {
        for cutoff in [1000.0, 18000.0, 23500.0] {
            let mut f = MoogLadder::new(48000.0, cutoff, 0.9).unwrap();
            for n in 0..48000 {
                assert!(f.process_sample((n as f32 * 0.73).sin() * 20.0).is_finite());
            }
        }
    }

    #[test]
    fn adaa_reduces_folded_third_harmonic() {
        let mut previous = 0.0;
        let mut naive = [0.0_f64; 2];
        let mut adaa = [0.0_f64; 2];
        for n in 0..8192 {
            let phase = std::f64::consts::TAU * 1703.0 * f64::from(n) / 8192.0;
            let x = (phase.sin() * 4.0) as f32;
            let y = saturate_adaa(x, &mut previous);
            let angle = 3.0 * phase;
            naive[0] += f64::from(tanh_approx(x)) * angle.cos();
            naive[1] += f64::from(tanh_approx(x)) * angle.sin();
            adaa[0] += f64::from(y) * angle.cos();
            adaa[1] += f64::from(y) * angle.sin();
        }
        assert!(adaa[0].hypot(adaa[1]) < naive[0].hypot(naive[1]) * 0.7);
    }
}
