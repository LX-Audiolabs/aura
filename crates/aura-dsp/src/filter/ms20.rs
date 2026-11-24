//! Korg MS-20 K35 approximation using two topology-preserving one-poles.
//!
//! Topology follows Stinchcombe's *A Study of the Korg MS10 & MS20 Filters*;
//! integrator discretization follows Zavalishin's *The Art of VA Filter Design*.
//! This is an original implementation, not copied source code.

use std::f32::consts::PI;

use crate::error::{self, DspError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ms20Revision {
    Diode,
    Ota,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ms20Mode {
    LowPass,
    HighPass,
}

#[derive(Debug, Clone)]
pub struct Ms20Filter {
    sample_rate: f32,
    cutoff: f32,
    resonance: f32,
    g: f32,
    s1: f32,
    s2: f32,
    mode: Ms20Mode,
    revision: Ms20Revision,
}

impl Ms20Filter {
    /// Create an OTA-revision low-pass filter.
    ///
    /// # Errors
    /// Returns an error for invalid sample rate, cutoff, or negative/non-finite resonance.
    pub fn new(sample_rate: f32, cutoff: f32, resonance: f32) -> Result<Self> {
        if let Some(err) = error::validate_sample_rate(sample_rate) {
            return Err(err);
        }
        if let Some(err) = error::validate_frequency(cutoff, sample_rate) {
            return Err(err);
        }
        validate_resonance(resonance)?;
        Ok(Self {
            sample_rate,
            cutoff,
            resonance: resonance.min(1.0),
            g: (PI * cutoff / sample_rate).tan(),
            s1: 0.0,
            s2: 0.0,
            mode: Ms20Mode::LowPass,
            revision: Ms20Revision::Ota,
        })
    }

    /// Update cutoff and resonance without changing integrator state.
    ///
    /// # Errors
    /// Returns an error for invalid cutoff or negative/non-finite resonance.
    pub fn set_params(&mut self, cutoff: f32, resonance: f32) -> Result<()> {
        if let Some(err) = error::validate_frequency(cutoff, self.sample_rate) {
            return Err(err);
        }
        validate_resonance(resonance)?;
        self.cutoff = cutoff;
        self.resonance = resonance.min(1.0);
        self.g = (PI * cutoff / self.sample_rate).tan();
        Ok(())
    }

    pub fn set_mode(&mut self, mode: Ms20Mode) {
        self.mode = mode;
    }

    pub fn set_revision(&mut self, revision: Ms20Revision) {
        self.revision = revision;
    }

    /// Solve the saturated band-pass feedback in the current sample. Both modes
    /// retain two TPT poles; the revision sets the feedback saturation voltage.
    pub fn process_sample(&mut self, input: f32) -> f32 {
        if !input.is_finite() {
            return 0.0;
        }
        let voltage = match self.revision {
            Ms20Revision::Diode => 0.33,
            Ms20Revision::Ota => 1.0,
        };
        let gain = self.g / (1.0 + self.g);
        let complement = 1.0 / (1.0 + self.g);
        let gamma = gain * complement;
        let sigma = match self.mode {
            Ms20Mode::LowPass => complement * (complement * self.s1 - self.s2),
            Ms20Mode::HighPass => complement * (self.s2 - gain * self.s1),
        };
        let resonance = 1.98 * self.resonance;
        let mut driven = input;
        // The derivative is >= 1 - 1.98/4 > 0.5. Fixed work, no convergence loop.
        for _ in 0..8 {
            let saturated = ((gamma * driven + sigma) / voltage).tanh();
            let residual = driven - input - resonance * voltage * saturated;
            let derivative = 1.0 - resonance * gamma * (1.0 - saturated * saturated);
            driven = (driven - residual / derivative)
                .clamp(input - resonance * voltage, input + resonance * voltage);
        }
        let lp1 = gain * driven + complement * self.s1;
        let second_input = match self.mode {
            Ms20Mode::LowPass => lp1,
            Ms20Mode::HighPass => driven - lp1,
        };
        let lp2 = gain * second_input + complement * self.s2;
        self.s1 = crate::flush_denormal(2.0 * lp1 - self.s1);
        self.s2 = crate::flush_denormal(2.0 * lp2 - self.s2);
        let output = match self.mode {
            Ms20Mode::LowPass => lp2,
            Ms20Mode::HighPass => second_input - lp2,
        };
        if output.is_finite() && self.s1.is_finite() && self.s2.is_finite() {
            output
        } else {
            self.reset();
            0.0
        }
    }

    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer {
            *sample = self.process_sample(*sample);
        }
    }

    pub fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }

    pub fn cutoff(&self) -> f32 {
        self.cutoff
    }
    pub fn resonance(&self) -> f32 {
        self.resonance
    }
    pub fn mode(&self) -> Ms20Mode {
        self.mode
    }
    pub fn revision(&self) -> Ms20Revision {
        self.revision
    }
}

fn validate_resonance(resonance: f32) -> Result<()> {
    if !resonance.is_finite() || resonance < 0.0 {
        return Err(DspError::InvalidFilterParameters);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Ms20Filter, Ms20Mode};

    #[test]
    fn ms20_new_valid() {
        assert!(Ms20Filter::new(44_100.0, 1_000.0, 0.3).is_ok());
    }
    #[test]
    fn ms20_new_bad_sr() {
        assert!(Ms20Filter::new(0.0, 1_000.0, 0.3).is_err());
    }
    #[test]
    fn ms20_new_bad_cutoff() {
        assert!(Ms20Filter::new(44_100.0, 30_000.0, 0.3).is_err());
    }
    #[test]
    fn ms20_lp_dc_settles() {
        let mut f = Ms20Filter::new(44_100.0, 8_000.0, 0.0).unwrap();
        f.set_mode(Ms20Mode::LowPass);
        let mut y: f32 = 0.0;
        for _ in 0..1_000 {
            y = f.process_sample(1.0);
        }
        assert!(y.is_finite() && y > 0.3);
    }
    #[test]
    fn ms20_hp_blocks_dc() {
        let mut f = Ms20Filter::new(44_100.0, 1_000.0, 0.0).unwrap();
        f.set_mode(Ms20Mode::HighPass);
        let mut y: f32 = 0.0;
        for _ in 0..1_000 {
            y = f.process_sample(1.0);
        }
        assert!(y.abs() < 0.05);
    }
    #[test]
    fn ms20_lp_hp_differ_on_sine() {
        let mut lp = Ms20Filter::new(44_100.0, 1_000.0, 0.0).unwrap();
        let mut hp = lp.clone();
        hp.set_mode(Ms20Mode::HighPass);
        let mut lp_energy: f32 = 0.0;
        let mut hp_energy: f32 = 0.0;
        for i in 0..512 {
            let x = (std::f32::consts::TAU * 100.0 * i as f32 / 44_100.0).sin();
            lp_energy += lp.process_sample(x).abs();
            hp_energy += hp.process_sample(x).abs();
        }
        assert!((lp_energy - hp_energy).abs() / 512.0 > 0.01);
    }
    #[test]
    fn ms20_high_res_finite() {
        let mut f = Ms20Filter::new(44_100.0, 1_000.0, 1.0).unwrap();
        for _ in 0..10_000 {
            assert!(f.process_sample(0.1).is_finite());
        }
    }
    #[test]
    fn ms20_nan_in_is_zero() {
        let mut f = Ms20Filter::new(44_100.0, 1_000.0, 0.3).unwrap();
        assert_eq!(f.process_sample(f32::NAN), 0.0);
    }
    #[test]
    fn ms20_reset_clears() {
        let mut f = Ms20Filter::new(44_100.0, 1_000.0, 0.3).unwrap();
        for _ in 0..100 {
            f.process_sample(1.0);
        }
        f.reset();
        assert!(f.process_sample(0.0).abs() < 1e-6);
    }
    #[test]
    fn ms20_set_params_invalid_preserves_cutoff() {
        let mut f = Ms20Filter::new(44_100.0, 1_000.0, 0.3).unwrap();
        assert!(f.set_params(0.0, 0.3).is_err());
        assert!((f.cutoff() - 1_000.0).abs() < f32::EPSILON);
    }
}

#[cfg(test)]
mod regressions {
    use super::*;
    #[test]
    fn ms20_cutoff_sweep_is_bounded_and_recovers() {
        for sr in [44100.0, 48000.0, 96000.0] {
            for ratio in [0.01, 0.2, 0.42, 0.49] {
                for mode in [Ms20Mode::LowPass, Ms20Mode::HighPass] {
                    let mut f = Ms20Filter::new(sr, sr * ratio, 1.0).unwrap();
                    f.set_mode(mode);
                    for i in 0..12000 {
                        let y = f.process_sample(0.1 * (i as f32 * 0.1).sin());
                        assert!(
                            y.is_finite() && y.abs() < 10.0,
                            "{sr} {ratio} {mode:?}: {y}"
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn ms20_highpass_resonance_and_revision_change_output() {
        let mut dry = Ms20Filter::new(48000.0, 1000.0, 0.0).unwrap();
        dry.set_mode(Ms20Mode::HighPass);
        let mut resonant = dry.clone();
        resonant.set_params(1000.0, 1.0).unwrap();
        let mut diode = resonant.clone();
        diode.set_revision(Ms20Revision::Diode);
        let (mut resonance_diff, mut revision_diff) = (0.0f32, 0.0f32);
        for i in 0..4000 {
            let x = (i as f32 * std::f32::consts::TAU * 1000.0 / 48000.0).sin();
            let a = dry.process_sample(x);
            let b = resonant.process_sample(x);
            let c = diode.process_sample(x);
            resonance_diff += (a - b).abs();
            revision_diff += (b - c).abs();
        }
        assert!(resonance_diff > 1.0 && revision_diff > 1.0);
    }
    #[test]
    fn ms20_highpass_has_two_pole_low_frequency_slope() {
        fn rms(hz: f32) -> f32 {
            let mut f = Ms20Filter::new(48000.0, 2000.0, 0.0).unwrap();
            f.set_mode(Ms20Mode::HighPass);
            let mut sum = 0.0;
            for i in 0..48000 {
                let y = f.process_sample((i as f32 * std::f32::consts::TAU * hz / 48000.0).sin());
                if i >= 24000 {
                    sum += y * y;
                }
            }
            (sum / 24000.0).sqrt()
        }
        let ratio = rms(200.0) / rms(100.0);
        assert!((3.8..4.2).contains(&ratio), "{ratio}");
    }
}

#[cfg(test)]
mod feedback_regressions {
    use super::*;
    #[test]
    fn implicit_feedback_matches_current_sample_bandpass() {
        for mode in [Ms20Mode::LowPass, Ms20Mode::HighPass] {
            let mut f = Ms20Filter::new(48000.0, 12000.0, 0.8).unwrap();
            f.set_mode(mode);
            for i in 0..100 {
                let x = 0.4 * (i as f32 * 0.2).sin();
                let old1 = f.s1;
                let old2 = f.s2;
                let y = f.process_sample(x);
                let lp1 = f32::midpoint(old1, f.s1);
                let lp2 = f32::midpoint(old2, f.s2);
                let gain = f.g / (1.0 + f.g);
                let c = 1.0 / (1.0 + f.g);
                let driven = (lp1 - c * old1) / gain;
                let bp = match mode {
                    Ms20Mode::LowPass => lp1 - y,
                    Ms20Mode::HighPass => lp2,
                };
                assert!(
                    (driven - x - 1.98 * 0.8 * bp.tanh()).abs() < 1e-5,
                    "{mode:?}"
                );
            }
        }
    }
}
