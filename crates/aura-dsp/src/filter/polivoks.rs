//! Polivoks-inspired nonlinear TPT SVF approximation.
//!
//! Own two-integrator loop keeps OTA-style saturation and slew limiting inside
//! the feedback path rather than applying a clipper after a linear SVF.

use std::f32::consts::PI;

use super::SvfOutput;
use crate::error::{self, DspError, Result};

#[derive(Debug, Clone)]
pub struct PolivoksFilter {
    sample_rate: f32,
    cutoff: f32,
    resonance: f32,
    drive: f32,
    g: f32,
    k: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    ic1eq: f32,
    ic2eq: f32,
}

impl PolivoksFilter {
    pub fn new(sample_rate: f32, cutoff: f32, resonance: f32, drive: f32) -> Result<Self> {
        if let Some(err) = error::validate_sample_rate(sample_rate) {
            return Err(err);
        }
        if let Some(err) = error::validate_frequency(cutoff, sample_rate) {
            return Err(err);
        }
        validate_resonance(resonance)?;
        validate_drive(drive)?;
        let mut filter = Self {
            sample_rate,
            cutoff,
            resonance,
            drive,
            g: 0.0,
            k: 1.0 / resonance,
            a1: 0.0,
            a2: 0.0,
            a3: 0.0,
            ic1eq: 0.0,
            ic2eq: 0.0,
        };
        filter.update_coefficients();
        Ok(filter)
    }

    pub fn set_params(&mut self, cutoff: f32, resonance: f32, drive: f32) -> Result<()> {
        if let Some(err) = error::validate_frequency(cutoff, self.sample_rate) {
            return Err(err);
        }
        validate_resonance(resonance)?;
        validate_drive(drive)?;
        self.cutoff = cutoff;
        self.resonance = resonance;
        self.drive = drive;
        self.update_coefficients();
        Ok(())
    }

    fn update_coefficients(&mut self) {
        self.g = (PI * self.cutoff / self.sample_rate).tan();
        self.k = 1.0 / self.resonance;
        self.a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
        self.a2 = self.g * self.a1;
        self.a3 = self.g * self.a2;
    }

    pub fn process_sample(&mut self, input: f32) -> SvfOutput {
        if !input.is_finite() {
            return zero_output();
        }
        let drive_eff =
            self.drive * (0.5 + 0.5 * (self.cutoff / (0.45 * self.sample_rate)).clamp(0.0, 1.0));
        let driven = (input * drive_eff).tanh();
        let v3 = driven - self.ic2eq - self.k * self.ic1eq;
        let v1 = self.a1 * self.ic1eq + self.a2 * v3;
        let v2 = self.ic2eq + self.a2 * self.ic1eq + self.a3 * v3;
        let slew = (2.0 * PI * self.cutoff / self.sample_rate).max(1.0e-6);
        let dv1 = (2.0 * v1 - self.ic1eq).clamp(-slew, slew);
        let dv2 = (2.0 * v2 - self.ic2eq).clamp(-slew, slew);
        self.ic1eq = crate::flush_denormal(self.ic1eq + dv1);
        self.ic2eq = crate::flush_denormal(self.ic2eq + dv2);
        let high_pass = driven - self.k * v1 - v2;
        let out = SvfOutput {
            low_pass: v2,
            high_pass,
            band_pass: v1,
            notch: high_pass + v2,
        };
        if out.low_pass.is_finite() && out.high_pass.is_finite() && out.band_pass.is_finite() {
            out
        } else {
            self.reset();
            zero_output()
        }
    }

    pub fn process_sample_lowpass(&mut self, input: f32) -> f32 {
        self.process_sample(input).low_pass
    }

    pub fn process_buffer_lowpass(&mut self, buffer: &mut [f32]) {
        for sample in buffer {
            *sample = self.process_sample_lowpass(*sample);
        }
    }

    pub fn reset(&mut self) {
        self.ic1eq = 0.0;
        self.ic2eq = 0.0;
    }

    pub fn cutoff(&self) -> f32 {
        self.cutoff
    }
    pub fn resonance(&self) -> f32 {
        self.resonance
    }
    pub fn drive(&self) -> f32 {
        self.drive
    }
}

fn validate_resonance(resonance: f32) -> Result<()> {
    if !resonance.is_finite() || resonance <= 0.0 {
        return Err(DspError::InvalidParameter {
            name: "resonance".into(),
            reason: "must be finite and > 0".into(),
        });
    }
    Ok(())
}

fn validate_drive(drive: f32) -> Result<()> {
    if !drive.is_finite() || drive <= 0.0 {
        return Err(DspError::InvalidParameter {
            name: "drive".into(),
            reason: "must be finite and > 0".into(),
        });
    }
    Ok(())
}

fn zero_output() -> SvfOutput {
    SvfOutput {
        low_pass: 0.0,
        high_pass: 0.0,
        band_pass: 0.0,
        notch: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::PolivoksFilter;

    #[test]
    fn polivoks_new_valid() {
        assert!(PolivoksFilter::new(44_100.0, 1_000.0, 0.707, 1.0).is_ok());
    }
    #[test]
    fn polivoks_new_bad_drive() {
        assert!(PolivoksFilter::new(44_100.0, 1_000.0, 0.707, 0.0).is_err());
    }
    #[test]
    fn polivoks_lp_dc_settles() {
        let mut f = PolivoksFilter::new(44_100.0, 8_000.0, 0.707, 1.0).unwrap();
        let mut y: f32 = 0.0;
        for _ in 0..1_000 {
            y = f.process_sample_lowpass(1.0);
        }
        assert!(y.is_finite() && y > 0.3);
    }
    #[test]
    fn polivoks_high_res_finite() {
        let mut f = PolivoksFilter::new(44_100.0, 10_000.0, 8.0, 1.0).unwrap();
        for _ in 0..10_000 {
            let y = f.process_sample(0.1);
            assert!(
                y.low_pass.is_finite()
                    && y.high_pass.is_finite()
                    && y.band_pass.is_finite()
                    && y.notch.is_finite()
            );
        }
    }
    #[test]
    fn polivoks_nan_in_is_zero() {
        let mut f = PolivoksFilter::new(44_100.0, 1_000.0, 0.707, 1.0).unwrap();
        let y = f.process_sample(f32::NAN);
        assert_eq!(
            (y.low_pass, y.high_pass, y.band_pass, y.notch),
            (0.0, 0.0, 0.0, 0.0)
        );
    }
    #[test]
    fn polivoks_drive_changes_rms() {
        let mut normal = PolivoksFilter::new(44_100.0, 4_000.0, 0.707, 1.0).unwrap();
        let mut driven = PolivoksFilter::new(44_100.0, 4_000.0, 0.707, 4.0).unwrap();
        let (mut a, mut b) = (0.0_f32, 0.0_f32);
        for i in 0..1_000 {
            let x = if i < 300 { 0.5 } else { 0.0 };
            a += normal.process_sample_lowpass(x).powi(2);
            b += driven.process_sample_lowpass(x).powi(2);
        }
        assert!((a - b).abs() > 1e-4);
    }
    #[test]
    fn polivoks_reset_clears() {
        let mut f = PolivoksFilter::new(44_100.0, 1_000.0, 0.707, 1.0).unwrap();
        for _ in 0..100 {
            f.process_sample(1.0);
        }
        f.reset();
        assert!(f.process_sample(0.0).low_pass.abs() < 1e-6);
    }
    #[test]
    fn polivoks_set_params_invalid_preserves_cutoff() {
        let mut f = PolivoksFilter::new(44_100.0, 1_000.0, 0.707, 1.0).unwrap();
        assert!(f.set_params(0.0, 0.7, 1.0).is_err());
        assert!((f.cutoff() - 1_000.0).abs() < f32::EPSILON);
    }
}
