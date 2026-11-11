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

    pub fn process_sample(&mut self, input: f32) -> f32 {
        let input = if input.is_finite() { input } else { 0.0 };
        let vd = match self.revision {
            Ms20Revision::Diode => 0.33,
            Ms20Revision::Ota => 1.0,
        };
        let feedback = self.s2 + vd * (3.0 * self.s2 / vd).tanh();
        let x = input - self.resonance * feedback;
        let v1 = self.g * (x - self.s1) / (1.0 + self.g);
        let lp1 = v1 + self.s1;
        self.s1 = crate::flush_denormal(lp1 + v1);

        let stage2_input = if self.mode == Ms20Mode::HighPass {
            input
        } else {
            lp1
        };
        let v2 = self.g * (stage2_input - self.s2) / (1.0 + self.g);
        let lp2 = v2 + self.s2;
        self.s2 = crate::flush_denormal(lp2 + v2);
        let output = match self.mode {
            Ms20Mode::LowPass => lp2,
            Ms20Mode::HighPass => input - lp2,
        };
        if output.is_finite() { output } else { 0.0 }
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
        return Err(DspError::InvalidParameter {
            name: "resonance".into(),
            reason: "must be finite and >= 0".into(),
        });
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
            let x = (std::f32::consts::TAU * 1_000.0 * i as f32 / 44_100.0).sin();
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
