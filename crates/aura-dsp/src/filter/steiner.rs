//! Three-feed Steiner-Parker Sallen-Key filter approximation.
//!
//! Original TPT implementation based on the documented two-integrator
//! Sallen-Key topology; nonlinear resonance is confined to the Q feedback path.

use std::f32::consts::PI;

use crate::error::{self, DspError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteinerMode {
    LowPass,
    BandPass,
    HighPass,
}

#[derive(Debug, Clone)]
pub struct SteinerParkerFilter {
    sample_rate: f32,
    cutoff: f32,
    resonance: f32,
    g: f32,
    s1: f32,
    s2: f32,
}

impl SteinerParkerFilter {
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
            resonance: resonance.min(1.5),
            g: (PI * cutoff / sample_rate).tan(),
            s1: 0.0,
            s2: 0.0,
        })
    }

    pub fn set_params(&mut self, cutoff: f32, resonance: f32) -> Result<()> {
        if let Some(err) = error::validate_frequency(cutoff, self.sample_rate) {
            return Err(err);
        }
        validate_resonance(resonance)?;
        self.cutoff = cutoff;
        self.resonance = resonance.min(1.5);
        self.g = (PI * cutoff / self.sample_rate).tan();
        Ok(())
    }

    /// Mix three feeds into the Sallen-Key node; return its shared low-pass output.
    pub fn process_sample(&mut self, lp: f32, bp: f32, hp: f32) -> f32 {
        if !lp.is_finite() || !bp.is_finite() || !hp.is_finite() {
            return 0.0;
        }
        self.step(lp + bp * 0.5 + hp * 0.25).1
    }

    pub fn process_mode(&mut self, input: f32, mode: SteinerMode) -> f32 {
        if !input.is_finite() {
            return 0.0;
        }
        let (lp, bp, hp) = match mode {
            SteinerMode::LowPass => (input, 0.0, 0.0),
            SteinerMode::BandPass => (0.0, input, 0.0),
            SteinerMode::HighPass => (0.0, 0.0, input),
        };
        let mixed = lp + bp * 0.5 + hp * 0.25;
        let (_, low, band, high) = self.step(mixed);
        match mode {
            SteinerMode::LowPass => low,
            SteinerMode::BandPass => band,
            SteinerMode::HighPass => high,
        }
    }

    fn step(&mut self, input: f32) -> (f32, f32, f32, f32) {
        let feedback = self.resonance * 0.5 * self.s2.tanh();
        let v1 = self.g * (input - self.s1) / (1.0 + self.g);
        let lp1 = v1 + self.s1;
        self.s1 = crate::flush_denormal(lp1 + v1);
        let v2 = self.g * (lp1 + feedback - self.s2) / (1.0 + self.g);
        let lp2 = v2 + self.s2;
        self.s2 = crate::flush_denormal(lp2 + v2);
        let high = input - lp2;
        let values = (lp1, lp2, lp1 - lp2, high);
        if values.1.is_finite() && values.0.is_finite() {
            values
        } else {
            self.reset();
            (0.0, 0.0, 0.0, 0.0)
        }
    }

    pub fn process_buffer_mode(&mut self, buffer: &mut [f32], mode: SteinerMode) {
        for sample in buffer {
            *sample = self.process_mode(*sample, mode);
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
    use super::{SteinerMode, SteinerParkerFilter};

    #[test]
    fn steiner_new_valid() {
        assert!(SteinerParkerFilter::new(44_100.0, 1_000.0, 0.3).is_ok());
    }
    #[test]
    fn steiner_new_bad_sr() {
        assert!(SteinerParkerFilter::new(0.0, 1_000.0, 0.3).is_err());
    }
    #[test]
    fn steiner_lp_dc_settles() {
        let mut f = SteinerParkerFilter::new(44_100.0, 8_000.0, 0.0).unwrap();
        let mut y: f32 = 0.0;
        for _ in 0..1_000 {
            y = f.process_mode(1.0, SteinerMode::LowPass);
        }
        assert!(y.is_finite() && y > 0.3);
    }
    #[test]
    fn steiner_hp_blocks_dc() {
        let mut f = SteinerParkerFilter::new(44_100.0, 1_000.0, 0.0).unwrap();
        let mut y: f32 = 0.0;
        for _ in 0..1_000 {
            y = f.process_mode(1.0, SteinerMode::HighPass);
        }
        assert!(y.abs() < 0.05);
    }
    #[test]
    fn steiner_three_inputs_differ() {
        let mut lp = SteinerParkerFilter::new(44_100.0, 1_000.0, 0.3).unwrap();
        let mut hp = lp.clone();
        for _ in 0..64 {
            lp.process_sample(1.0, 0.0, 0.0);
        }
        let a = lp.process_sample(1.0, 0.0, 0.0);
        for _ in 0..64 {
            hp.process_sample(0.0, 0.0, 1.0);
        }
        let b = hp.process_sample(0.0, 0.0, 1.0);
        assert!((a - b).abs() > 1e-5);
    }
    #[test]
    fn steiner_high_res_finite() {
        let mut f = SteinerParkerFilter::new(44_100.0, 10_000.0, 1.5).unwrap();
        for _ in 0..10_000 {
            assert!(f.process_mode(0.1, SteinerMode::LowPass).is_finite());
        }
    }
    #[test]
    fn steiner_nan_in_is_zero() {
        let mut f = SteinerParkerFilter::new(44_100.0, 1_000.0, 0.3).unwrap();
        assert_eq!(f.process_sample(f32::NAN, 0.0, 0.0), 0.0);
        assert_eq!(f.process_sample(0.0, f32::INFINITY, 0.0), 0.0);
        assert_eq!(f.process_sample(0.0, 0.0, f32::NEG_INFINITY), 0.0);
    }
    #[test]
    fn steiner_reset_clears() {
        let mut f = SteinerParkerFilter::new(44_100.0, 1_000.0, 0.3).unwrap();
        for _ in 0..100 {
            f.process_mode(1.0, SteinerMode::LowPass);
        }
        f.reset();
        assert!(f.process_mode(0.0, SteinerMode::LowPass).abs() < 1e-6);
    }
    #[test]
    fn steiner_set_params_invalid_preserves_cutoff() {
        let mut f = SteinerParkerFilter::new(44_100.0, 1_000.0, 0.3).unwrap();
        assert!(f.set_params(0.0, 0.3).is_err());
        assert!((f.cutoff() - 1_000.0).abs() < f32::EPSILON);
    }
}
