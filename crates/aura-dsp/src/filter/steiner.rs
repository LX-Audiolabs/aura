//! Three-feed Steiner-Parker Sallen-Key filter approximation.
//!
//! Shared second-order pole pair (bilinear, prewarped g). Each jack applies a
//! different zero set into that resonator; one shared output node. Resonance
//! approaches small-signal Q=50 at resonance 1.5; delayed amplitude-dependent
//! damping compresses the peak and generates harmonics under drive. This is a
//! heuristic coloration, not a circuit-validated model of Steiner nonlinearities.
//! The DF1 recursion uses double precision to preserve low-cutoff DC gain.
//!
//! No internal antialiasing is applied. When oversampling, run the entire filter
//! at the higher sample rate with filtered upsampling and downsampling.

use std::f64::consts::PI;

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
    g: f64,
    /// Shared output delays y[n-1], y[n-2] (DF1 recursive part).
    y1: f64,
    y2: f64,
    /// Feed-forward input history (three jacks × two delays).
    lp1: f64,
    lp2: f64,
    bp1: f64,
    bp2: f64,
    hp1: f64,
    hp2: f64,
}

impl SteinerParkerFilter {
    /// Create a three-feed Sallen-Key filter.
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
            resonance: resonance.min(1.5),
            g: (PI * f64::from(cutoff) / f64::from(sample_rate)).tan(),
            y1: 0.0,
            y2: 0.0,
            lp1: 0.0,
            lp2: 0.0,
            bp1: 0.0,
            bp2: 0.0,
            hp1: 0.0,
            hp2: 0.0,
        })
    }

    /// Update cutoff and resonance without changing filter state.
    ///
    /// # Errors
    /// Returns an error for invalid cutoff or negative/non-finite resonance.
    pub fn set_params(&mut self, cutoff: f32, resonance: f32) -> Result<()> {
        if let Some(err) = error::validate_frequency(cutoff, self.sample_rate) {
            return Err(err);
        }
        validate_resonance(resonance)?;
        self.cutoff = cutoff;
        self.resonance = resonance.min(1.5);
        self.g = (PI * f64::from(cutoff) / f64::from(self.sample_rate)).tan();
        Ok(())
    }

    /// Mix three feeds into the shared SK output node.
    pub fn process_sample(&mut self, lp: f32, bp: f32, hp: f32) -> f32 {
        if !lp.is_finite() || !bp.is_finite() || !hp.is_finite() {
            return 0.0;
        }
        self.step(lp, bp, hp)
    }

    /// Send `input` to one mode jack, zeros to the others; shared output.
    pub fn process_mode(&mut self, input: f32, mode: SteinerMode) -> f32 {
        if !input.is_finite() {
            return 0.0;
        }
        match mode {
            SteinerMode::LowPass => self.step(input, 0.0, 0.0),
            SteinerMode::BandPass => self.step(0.0, input, 0.0),
            SteinerMode::HighPass => self.step(0.0, 0.0, input),
        }
    }

    /// Continuous idea (warped):
    /// `Y = (ω² L + s ω B + s² H) / (s² + d ω s + ω²)`.
    /// Digital monic DF1 with shared poles; numerators per jack.
    fn step(&mut self, lp: f32, bp: f32, hp: f32) -> f32 {
        let (lp, bp, hp) = (f64::from(lp), f64::from(bp), f64::from(hp));
        let resonance = f64::from(self.resonance);
        let g = self.g;
        let g2 = g * g;
        // Bend the upper resonance range toward oscillation: d=2 → 0.02 (Q=50).
        let d_lin = 2.0 - resonance - 0.48 * (resonance / 1.5).powi(4);
        // Symmetric nonlinear loss limits ringing without changing the jack zeros.
        let soft = self.y1.tanh();
        let d = (d_lin + 0.35 * resonance * soft * soft).clamp(0.02, 2.0);

        let inv = 1.0 / (1.0 + d * g + g2);
        // Shared poles (monic a0 = 1 after scale):
        let a1 = 2.0 * (g2 - 1.0) * inv;
        let a2 = (1.0 - d * g + g2) * inv;
        // Zero sets (scaled by inv into b*):
        // LP ~ g² (1+z⁻¹)², BP ~ g (1 − z⁻²), HP ~ (1 − z⁻¹)²
        let bl0 = g2 * inv;
        let bl1 = 2.0 * g2 * inv;
        let bl2 = g2 * inv;
        let bb0 = g * inv;
        let bb1 = 0.0;
        let bb2 = -g * inv;
        let bh0 = inv;
        let bh1 = -2.0 * inv;
        let bh2 = inv;

        let mut y = bl0 * lp
            + bl1 * self.lp1
            + bl2 * self.lp2
            + bb0 * bp
            + bb1 * self.bp1
            + bb2 * self.bp2
            + bh0 * hp
            + bh1 * self.hp1
            + bh2 * self.hp2
            - a1 * self.y1
            - a2 * self.y2;

        let output = y as f32;
        if !output.is_finite() {
            self.reset();
            return 0.0;
        }
        if y.abs() < f64::MIN_POSITIVE {
            y = 0.0;
        }

        self.lp2 = self.lp1;
        self.lp1 = lp;
        self.bp2 = self.bp1;
        self.bp1 = bp;
        self.hp2 = self.hp1;
        self.hp1 = hp;
        self.y2 = self.y1;
        self.y1 = y;
        crate::flush_denormal(output)
    }

    pub fn process_buffer_mode(&mut self, buffer: &mut [f32], mode: SteinerMode) {
        for sample in buffer {
            *sample = self.process_mode(*sample, mode);
        }
    }

    pub fn reset(&mut self) {
        self.y1 = 0.0;
        self.y2 = 0.0;
        self.lp1 = 0.0;
        self.lp2 = 0.0;
        self.bp1 = 0.0;
        self.bp2 = 0.0;
        self.hp1 = 0.0;
        self.hp2 = 0.0;
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
        return Err(DspError::InvalidFilterParameters);
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
        assert!((y - 1.0).abs() < 0.001, "LP DC gain: {y}");
    }
    #[test]
    fn steiner_low_cutoff_preserves_dc_gain() {
        for sample_rate in [44_100.0, 48_000.0, 96_000.0] {
            for cutoff in [1.0, 20.0] {
                for resonance in [0.0, 1.5] {
                    let constructed =
                        SteinerParkerFilter::new(sample_rate, cutoff, resonance).unwrap();
                    let mut updated = SteinerParkerFilter::new(sample_rate, 1_000.0, 0.3).unwrap();
                    updated.set_params(cutoff, resonance).unwrap();
                    // High resonance at 1 Hz needs longer to settle its ringing.
                    let seconds = if resonance > 0.0 { 60 } else { 10 };
                    for mut f in [constructed, updated] {
                        let mut y = 0.0;
                        for _ in 0..(sample_rate as usize * seconds) {
                            y = f.process_mode(1.0, SteinerMode::LowPass);
                        }
                        assert!(
                            (y - 1.0).abs() < 0.001,
                            "DC gain: sample_rate={sample_rate}, cutoff={cutoff}, resonance={resonance}, output={y}"
                        );
                    }
                }
            }
        }
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
    fn steiner_high_res_has_sharp_peak_and_harmonics() {
        let mut gains = [0.0_f64; 2];
        let mut third_ratio = 0.0;
        for (index, amplitude) in [0.001_f32, 0.1].into_iter().enumerate() {
            let mut f = SteinerParkerFilter::new(48_000.0, 1_000.0, 1.5).unwrap();
            let (mut fundamental, mut third) = (0.0, 0.0);
            let (mut fundamental_cos, mut third_cos) = (0.0, 0.0);
            for i in 0..48_000 {
                let phase = std::f64::consts::TAU * f64::from(i) / 48.0;
                let y =
                    f64::from(f.process_mode(amplitude * phase.sin() as f32, SteinerMode::LowPass));
                if i >= 24_000 {
                    fundamental += y * phase.sin();
                    fundamental_cos += y * phase.cos();
                    third += y * (3.0 * phase).sin();
                    third_cos += y * (3.0 * phase).cos();
                }
            }
            let fundamental = fundamental.hypot(fundamental_cos) / 12_000.0;
            gains[index] = fundamental / f64::from(amplitude);
            third_ratio = third.hypot(third_cos) / 12_000.0 / fundamental;
        }
        let mut impulse = SteinerParkerFilter::new(48_000.0, 1_000.0, 1.5).unwrap();
        let mut tail_energy = 0.0;
        for i in 0..4_800 {
            let y = f64::from(
                impulse.process_mode(if i == 0 { 1.0 } else { 0.0 }, SteinerMode::LowPass),
            );
            if i >= 2_400 {
                tail_energy += y * y;
            }
        }
        eprintln!(
            "Steiner res=1.5: small gain={}, driven gain={}, H3/fundamental={}, impulse late energy={tail_energy}",
            gains[0], gains[1], third_ratio
        );
        assert!(
            gains[0] > 20.0,
            "small-signal resonance must exceed Q=2 baseline: {gains:?}"
        );
        assert!(
            gains[1] > 3.0 && gains[1] < gains[0],
            "driven resonance must remain loud and compress: {gains:?}"
        );
        assert!(
            third_ratio > 0.002,
            "resonance must generate harmonics: {third_ratio}"
        );
        assert!(
            tail_energy > 1e-6,
            "impulse must ring beyond 50 ms: {tail_energy}"
        );
    }

    #[test]
    fn steiner_extreme_cutoffs_and_inputs_stay_finite() {
        for sample_rate in [44_100.0, 48_000.0, 96_000.0] {
            let below_nyquist = f32::from_bits((sample_rate * 0.5_f32).to_bits() - 1);
            for cutoff in [1.0, 20.0, 1_000.0, sample_rate * 0.49, below_nyquist] {
                for mode in [
                    SteinerMode::LowPass,
                    SteinerMode::BandPass,
                    SteinerMode::HighPass,
                ] {
                    let mut f = SteinerParkerFilter::new(sample_rate, cutoff, 1.5).unwrap();
                    for i in 0..48_000 {
                        let input = if i < 12_000 {
                            1.0
                        } else {
                            (f64::from(i) * 0.13).sin() as f32
                        };
                        let y = f.process_mode(input, mode);
                        assert!(
                            y.is_finite() && y.abs() < 100.0,
                            "sr={sample_rate} cutoff={cutoff} mode={mode:?} output={y}"
                        );
                    }
                    assert_eq!(f.process_mode(f32::NAN, mode), 0.0);
                    assert_eq!(f.process_sample(0.0, f32::NAN, 0.0), 0.0);
                    assert!(f.process_sample(f32::MAX, -f32::MAX, f32::MAX).is_finite());
                    assert!(f.process_mode(0.0, mode).is_finite());
                }
            }
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
    #[test]
    fn steiner_process_mode_matches_process_sample_jack() {
        let mut a = SteinerParkerFilter::new(44_100.0, 2_000.0, 0.4).unwrap();
        let mut b = a.clone();
        let mut max_diff = 0.0_f32;
        for i in 0..256 {
            let x = (std::f32::consts::TAU * 440.0 * i as f32 / 44_100.0).sin() * 0.5;
            let ya = a.process_mode(x, SteinerMode::HighPass);
            let yb = b.process_sample(0.0, 0.0, x);
            max_diff = max_diff.max((ya - yb).abs());
        }
        assert!(max_diff < 1e-5, "mode vs jack max diff {max_diff}");
    }
}
