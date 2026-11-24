//! Discrete Summation Formula (DSF) oscillators.
//!
//! Ported from fundsp `src/oscillator.rs` (MIT / Apache-2.0).
//! Original authors: Sami Perttu and contributors.
//! Algorithm: Moorer, J. A., “The Synthesis of Complex Audio Spectra by Means
//! of Discrete Summation Formulae”, 1976.
//!
//! DSF produces bandlimited saw / square via a closed-form partial sum
//! (O(1) per sample). Number of partials is limited by Nyquist so the
//! result stays alias-free within the sample rate.
//!
//! Use DSF when pristine quality matters; use `PolyBLEP` for efficiency.

use std::f32::consts::TAU;

use crate::error::{self, Result};

/// DSF saw-like oscillator (all harmonics, spacing = 1).
///
/// Roughness `r` (clamped to ~0…1) is the relative amplitude of successive
/// partials — higher retains more high-frequency energy.
#[derive(Debug, Clone)]
pub struct DsfSaw {
    phase: f32,
    roughness: f32,
    sample_rate: f32,
    sample_duration: f32,
}

impl DsfSaw {
    /// Create a DSF saw oscillator.
    ///
    /// # Errors
    ///
    /// Returns error if `sample_rate` is invalid.
    pub fn new(sample_rate: f32, roughness: f32) -> Result<Self> {
        if let Some(e) = error::validate_sample_rate(sample_rate) {
            return Err(e);
        }
        Ok(Self {
            phase: 0.0,
            roughness: clamp_roughness(roughness),
            sample_rate,
            sample_duration: 1.0 / sample_rate,
        })
    }

    /// Create with initial phase in 0…1.
    ///
    /// # Errors
    ///
    /// Returns error if `sample_rate` is invalid.
    pub fn with_phase(sample_rate: f32, roughness: f32, phase: f32) -> Result<Self> {
        let mut osc = Self::new(sample_rate, roughness)?;
        osc.phase = if phase.is_finite() {
            phase.rem_euclid(1.0)
        } else {
            0.0
        };
        Ok(osc)
    }

    /// Set roughness (0…1). Higher = brighter.
    #[inline]
    pub fn set_roughness(&mut self, roughness: f32) {
        self.roughness = clamp_roughness(roughness);
    }

    /// Current roughness.
    #[inline]
    #[must_use]
    pub fn roughness(&self) -> f32 {
        self.roughness
    }

    /// Reset phase to 0.
    #[inline]
    pub fn reset(&mut self) {
        self.phase = 0.0;
    }

    /// Generate next sample at the given frequency (Hz).
    #[inline]
    pub fn next_sample(&mut self, frequency: f32) -> f32 {
        dsf_tick(
            &mut self.phase,
            self.sample_duration,
            self.sample_rate,
            frequency,
            1.0,
            self.roughness,
        )
    }

    /// Fill a buffer at constant frequency.
    #[inline]
    pub fn process_buffer(&mut self, frequency: f32, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.next_sample(frequency);
        }
    }
}

/// DSF square-like oscillator (odd harmonics only, spacing = 2).
#[derive(Debug, Clone)]
pub struct DsfSquare {
    phase: f32,
    roughness: f32,
    sample_rate: f32,
    sample_duration: f32,
}

impl DsfSquare {
    /// Create a DSF square oscillator.
    ///
    /// # Errors
    ///
    /// Returns error if `sample_rate` is invalid.
    pub fn new(sample_rate: f32, roughness: f32) -> Result<Self> {
        if let Some(e) = error::validate_sample_rate(sample_rate) {
            return Err(e);
        }
        Ok(Self {
            phase: 0.0,
            roughness: clamp_roughness(roughness),
            sample_rate,
            sample_duration: 1.0 / sample_rate,
        })
    }

    /// Create with initial phase in 0…1.
    ///
    /// # Errors
    ///
    /// Returns error if `sample_rate` is invalid.
    pub fn with_phase(sample_rate: f32, roughness: f32, phase: f32) -> Result<Self> {
        let mut osc = Self::new(sample_rate, roughness)?;
        osc.phase = if phase.is_finite() {
            phase.rem_euclid(1.0)
        } else {
            0.0
        };
        Ok(osc)
    }

    /// Set roughness (0…1).
    #[inline]
    pub fn set_roughness(&mut self, roughness: f32) {
        self.roughness = clamp_roughness(roughness);
    }

    /// Current roughness.
    #[inline]
    #[must_use]
    pub fn roughness(&self) -> f32 {
        self.roughness
    }

    /// Reset phase to 0.
    #[inline]
    pub fn reset(&mut self) {
        self.phase = 0.0;
    }

    /// Generate next sample at the given frequency (Hz).
    #[inline]
    pub fn next_sample(&mut self, frequency: f32) -> f32 {
        dsf_tick(
            &mut self.phase,
            self.sample_duration,
            self.sample_rate,
            frequency,
            2.0,
            self.roughness,
        )
    }

    /// Fill a buffer at constant frequency.
    #[inline]
    pub fn process_buffer(&mut self, frequency: f32, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.next_sample(frequency);
        }
    }
}

/// fundsp clamps roughness away from the singular endpoints 0 and 1.
#[inline]
fn clamp_roughness(r: f32) -> f32 {
    if r.is_finite() {
        r.clamp(0.0001, 0.9999)
    } else {
        0.5
    }
}

/// Advance phase and evaluate DSF (fundsp `Dsf::tick` logic).
#[inline]
fn dsf_tick(
    phase: &mut f32,
    sample_duration: f32,
    sample_rate: f32,
    frequency: f32,
    harmonic_spacing: f32,
    roughness: f32,
) -> f32 {
    if !frequency.is_finite() || frequency <= 0.0 || frequency >= sample_rate * 0.5 {
        return 0.0;
    }
    let freq = frequency;
    *phase = (*phase + freq * sample_duration).rem_euclid(1.0);

    // Index i is inclusive; partial i has frequency (1 + i * spacing) * freq.
    let last = (((f64::from(sample_rate) * 0.5 / f64::from(freq)) - 1.0)
        / f64::from(harmonic_spacing))
    .ceil()
        - 1.0;
    // Beyond this index even the brightest supported roughness is negligible.
    let n = last.clamp(0.0, 1_000_000.0) as f32;

    // f = phase in radians; d = increment per partial index (fundsp passes f * spacing).
    let f = *phase * TAU;
    let d = f * harmonic_spacing;
    dsf(f, d, roughness, n)
}

/// Discrete Summation Formula (closed form).
///
/// Computes `sum_{i=0..n} r^i * sin(f + i * d)`.
#[inline]
fn dsf(f: f32, d: f32, r: f32, n: f32) -> f32 {
    let (f, d, r, n) = (f64::from(f), f64::from(d), f64::from(r), f64::from(n));
    let rn1 = r.powf(n + 1.0);
    // Equivalent to 1+r²-2r*cos(d), without cancellation near r=1, d=0.
    let denom = (1.0 - r).powi(2) + 4.0 * r * (d * 0.5).sin().powi(2);
    let num =
        f.sin() - r * (f - d).sin() - rn1 * ((f + (n + 1.0) * d).sin() - r * (f + n * d).sin());
    (num / denom) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dsf_saw_produces_audio() {
        let mut osc = DsfSaw::new(44_100.0, 0.5).unwrap();
        let mut sum = 0.0f32;
        for _ in 0..100 {
            let s = osc.next_sample(440.0);
            assert!(s.is_finite());
            sum += s.abs();
        }
        assert!(sum > 0.0, "should produce non-silent output");
    }

    #[test]
    fn dsf_square_produces_audio() {
        let mut osc = DsfSquare::new(44_100.0, 0.5).unwrap();
        let mut sum = 0.0f32;
        for _ in 0..100 {
            let s = osc.next_sample(440.0);
            assert!(s.is_finite());
            sum += s.abs();
        }
        assert!(sum > 0.0);
    }

    #[test]
    fn dsf_saw_invalid_sr() {
        assert!(DsfSaw::new(0.0, 0.5).is_err());
    }

    #[test]
    fn dsf_saw_high_roughness_not_silent() {
        // Old bug: * (1 - roughness) zeroed the output at r→1.
        let mut osc = DsfSaw::new(44_100.0, 0.99).unwrap();
        let mut sum = 0.0f32;
        for _ in 0..256 {
            sum += osc.next_sample(220.0).abs();
        }
        assert!(
            sum > 1.0,
            "bright DSF must not collapse to silence, sum={sum}"
        );
    }

    #[test]
    fn dsf_closed_form_matches_naive_sum() {
        // Spot-check closed form vs direct sum at fixed args.
        let f = 0.3f32 * TAU;
        let d = f; // spacing 1
        let r = 0.5f32;
        let n = 8.0f32;
        let closed = dsf(f, d, r, n);
        let mut naive = 0.0f32;
        let mut amp = 1.0f32;
        for i in 0..=8 {
            naive += amp * (f + i as f32 * d).sin();
            amp *= r;
        }
        assert!(
            (closed - naive).abs() < 1e-4,
            "closed={closed} naive={naive}"
        );
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;

    #[test]
    fn regression_partial_limit() {
        for spacing in [1.0, 2.0] {
            for freq in [8000.0, 10_000.0, 15_000.0, 24_000.0, 30_000.0] {
                let mut phase = 0.0;
                for _ in 0..48 {
                    let y = dsf_tick(&mut phase, 1.0 / 48_000.0, 48_000.0, freq, spacing, 0.8);
                    let expected: f64 = (0..64)
                        .filter(|i| (1.0 + *i as f32 * spacing) * freq < 24_000.0)
                        .map(|i| {
                            0.8_f64.powi(i)
                                * (f64::from(phase)
                                    * std::f64::consts::TAU
                                    * (1.0 + f64::from(i) * f64::from(spacing)))
                                .sin()
                        })
                        .sum();
                    assert!(
                        (f64::from(y) - expected).abs() < 1e-4,
                        "{freq} {spacing}: {y} vs {expected}"
                    );
                }
            }
        }
    }
    #[test]
    fn regression_near_unity_roughness() {
        let f = 0.00001;
        let expected: f64 = (0..=8)
            .map(|i| 0.9999_f64.powi(i) * (f64::from(f) * f64::from(i + 1)).sin())
            .sum();
        assert!((f64::from(dsf(f, f, 0.9999, 8.0)) - expected).abs() < 1e-6);
    }
}
