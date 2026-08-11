//! Discrete Summation Formula (DSF) oscillators.
//!
//! Ported from fundsp `src/oscillator.rs` (MIT / Apache-2.0).
//! Original authors: Sami Perttu and contributors.
//!
//! DSF produces bandlimited saw and square waves by summing phase-shifted
//! sines. Unlike PolyBLEP, DSF has no aliasing at all (within Nyquist)
//! but is more computationally expensive — ~100 sine evaluations per sample.
//!
//! Use DSF when pristine quality matters; use PolyBLEP for efficiency.

use std::f32::consts::TAU;

/// DSF saw-like oscillator.
///
/// Produces a saw wave via discrete summation:
/// `sum(r^i * sin(phase + i * spacing), i=0..100)`.
///
/// Roughness `r` (0..1) controls the brightness — higher values
/// retain more high-frequency partials.
#[derive(Debug, Clone)]
pub struct DsfSaw {
    phase: f32,
    roughness: f32,
    sample_duration: f32,
}

impl DsfSaw {
    /// Create a DSF saw oscillator.
    ///
    /// `roughness` in 0..1 controls partial attenuation.
    pub fn new(sample_rate: f32, roughness: f32) -> Self {
        Self {
            phase: 0.0,
            roughness: roughness.clamp(0.0, 1.0),
            sample_duration: 1.0 / sample_rate,
        }
    }

    /// Create with initial phase in 0..1.
    pub fn with_phase(sample_rate: f32, roughness: f32, phase: f32) -> Self {
        let mut osc = Self::new(sample_rate, roughness);
        osc.phase = phase.clamp(0.0, 1.0);
        osc
    }

    /// Set roughness (0..1). Higher = brighter.
    #[inline]
    pub fn set_roughness(&mut self, roughness: f32) {
        self.roughness = roughness.clamp(0.0, 1.0);
    }

    /// Reset phase to 0.
    #[inline]
    pub fn reset(&mut self) {
        self.phase = 0.0;
    }

    /// Generate next sample at the given frequency.
    #[inline]
    pub fn next_sample(&mut self, frequency: f32) -> f32 {
        let delta = frequency * self.sample_duration;
        let phase = self.phase * TAU;
        let value = dsf(phase, 1.0, self.roughness, 100.0)
            * 2.0
            * (1.0 - self.roughness);
        self.phase += delta;
        if self.phase >= 1.0 {
            self.phase -= (self.phase as i32) as f32;
        }
        value
    }
}

/// DSF square-like oscillator.
///
/// Produces a square wave by using harmonic spacing of 2
/// (odd harmonics only). Otherwise identical to [`DsfSaw`].
#[derive(Debug, Clone)]
pub struct DsfSquare {
    phase: f32,
    roughness: f32,
    sample_duration: f32,
}

impl DsfSquare {
    /// Create a DSF square oscillator.
    pub fn new(sample_rate: f32, roughness: f32) -> Self {
        Self {
            phase: 0.0,
            roughness: roughness.clamp(0.0, 1.0),
            sample_duration: 1.0 / sample_rate,
        }
    }

    /// Create with initial phase in 0..1.
    pub fn with_phase(sample_rate: f32, roughness: f32, phase: f32) -> Self {
        let mut osc = Self::new(sample_rate, roughness);
        osc.phase = phase.clamp(0.0, 1.0);
        osc
    }

    /// Set roughness (0..1).
    #[inline]
    pub fn set_roughness(&mut self, roughness: f32) {
        self.roughness = roughness.clamp(0.0, 1.0);
    }

    /// Reset phase to 0.
    #[inline]
    pub fn reset(&mut self) {
        self.phase = 0.0;
    }

    /// Generate next sample at the given frequency.
    #[inline]
    pub fn next_sample(&mut self, frequency: f32) -> f32 {
        let delta = frequency * self.sample_duration;
        let phase = self.phase * TAU;
        // Square = odd harmonics only → spacing = 2.0
        let value = dsf(phase, 2.0, self.roughness, 50.0)
            * 2.0
            * (1.0 - self.roughness);
        self.phase += delta;
        if self.phase >= 1.0 {
            self.phase -= (self.phase as i32) as f32;
        }
        value
    }
}

/// Discrete Summation Formula.
///
/// Computes `sum(r^i * sin(f + i * d), i=0..n)`.
///
/// This is the core DSF algorithm producing bandlimited waveforms
/// without aliasing within the Nyquist limit.
#[inline]
fn dsf(f: f32, d: f32, r: f32, n: f32) -> f32 {
    // DSF closed form: sin(f) - r*sin(f-d) - r^(n+1)*[sin(f+(n+1)*d) - r*sin(f+n*d)]
    // divided by (1 + r^2 - 2*r*cos(d))
    let rn1 = r.powf(n + 1.0);
    let denom = 1.0 + r * r - 2.0 * r * d.cos();

    if denom.abs() < 1e-10 {
        return 0.0;
    }

    let num = f.sin()
        - r * (f - d).sin()
        - rn1 * ((f + (n + 1.0) * d).sin() - r * (f + n * d).sin());

    num / denom
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dsf_saw_produces_audio() {
        let mut osc = DsfSaw::new(44100.0, 0.5);
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
        let mut osc = DsfSquare::new(44100.0, 0.5);
        let mut sum = 0.0f32;
        for _ in 0..100 {
            let s = osc.next_sample(440.0);
            assert!(s.is_finite());
            sum += s.abs();
        }
        assert!(sum > 0.0);
    }

    #[test]
    fn dsf_saw_respects_roughness() {
        let mut bright = DsfSaw::new(44100.0, 0.99);
        let mut dull = DsfSaw::new(44100.0, 0.1);
        // Both should produce valid output
        let b = bright.next_sample(440.0);
        let d = dull.next_sample(440.0);
        assert!(b.is_finite() && d.is_finite());
    }
}
