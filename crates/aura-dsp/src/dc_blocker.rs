//! One-pole DC blocker for bus / master paths.
//!
//! `y[n] = x[n] - x[n-1] + R * y[n-1]` with default `R = 0.995`.
//! Product plugins may still apply exact-zero snap after this for dry silence tests.

use serde::{Deserialize, Serialize};

const DEFAULT_R: f32 = 0.995;

/// One-pole DC blocker. RT-safe after construction; no alloc in [`Self::process`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DcBlocker {
    r: f32,
    x1: f32,
    y1: f32,
}

impl Default for DcBlocker {
    fn default() -> Self {
        Self::new()
    }
}

impl DcBlocker {
    /// Create with default coefficient `R = 0.995`.
    #[must_use]
    pub fn new() -> Self {
        Self::with_r(DEFAULT_R)
    }

    /// Create with custom `R` in `(0, 1)`. Non-finite or out-of-range → `0.995`.
    #[must_use]
    pub fn with_r(r: f32) -> Self {
        let r = if r.is_finite() && r > 0.0 && r < 1.0 {
            r
        } else {
            DEFAULT_R
        };
        Self {
            r,
            x1: 0.0,
            y1: 0.0,
        }
    }

    /// Clear delay state (does not change `R`).
    #[inline]
    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.y1 = 0.0;
    }

    /// Process one sample. Non-finite input → `0.0`. Denormals flushed on state.
    #[inline]
    #[must_use]
    pub fn process(&mut self, x: f32) -> f32 {
        let x = crate::flush_denormal(if x.is_finite() { x } else { 0.0 });
        let y = x - self.x1 + self.r * self.y1;
        let y = crate::flush_denormal(y);
        self.x1 = x;
        self.y1 = y;
        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_dc_decays_toward_zero() {
        // Step response with constant DC is y[n] = R^n; need n ≳ 600 for |y|<0.05.
        let mut dc = DcBlocker::new();
        let mut last = 0.0f32;
        for _ in 0..1_000 {
            last = dc.process(1.0);
        }
        assert!(
            last.abs() < 0.05,
            "steady DC must be attenuated; last={last}"
        );
    }

    #[test]
    fn silence_stays_near_zero() {
        let mut dc = DcBlocker::new();
        let _ = dc.process(1.0);
        // After impulse, residual |y| ≈ 0.005 * R^n — give the pole time to die.
        for _ in 0..2_000 {
            let y = dc.process(0.0);
            assert!(y.is_finite());
        }
        let y = dc.process(0.0);
        assert!(y.abs() < 1e-3, "settled silence must be near 0, got {y}");
    }

    #[test]
    fn reset_clears_state() {
        let mut dc = DcBlocker::new();
        for _ in 0..50 {
            let _ = dc.process(0.5);
        }
        dc.reset();
        let y = dc.process(0.0);
        assert!(
            y.abs() < 1e-6,
            "after reset silence input must be ~0, got {y}"
        );
    }

    #[test]
    fn non_finite_input_becomes_zero() {
        let mut dc = DcBlocker::new();
        let y = dc.process(f32::NAN);
        assert_eq!(y, 0.0);
        let y = dc.process(f32::INFINITY);
        assert!(y.is_finite());
    }

    #[test]
    fn subnormal_input_is_flushed_from_state() {
        let mut dc = DcBlocker::new();
        assert_eq!(dc.process(f32::from_bits(1)), 0.0);
        assert_eq!(dc.x1, 0.0);
    }

    #[test]
    fn with_r_rejects_bad_coeff() {
        let a = DcBlocker::with_r(f32::NAN);
        let b = DcBlocker::with_r(1.5);
        let c = DcBlocker::with_r(0.0);
        // Smoke: all construct and process finite.
        for mut d in [a, b, c] {
            let y = d.process(0.25);
            assert!(y.is_finite());
        }
    }
}
