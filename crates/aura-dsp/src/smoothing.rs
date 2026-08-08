//! Parameter smoothing for click-free transitions.
//!
//! Provides exponential moving average (EMA) smoothing to prevent audible
//! clicks and zipper noise when parameters change during playback.

use serde::{Deserialize, Serialize};

/// Exponential parameter smoother (one-pole lowpass).
///
/// Smooths parameter changes over a configurable time to prevent
/// audible clicks. The smoothing coefficient is derived from the
/// time constant and sample rate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamSmoother {
    /// Current smoothed value.
    current: f32,
    /// Target value.
    target: f32,
    /// Smoothing coefficient (0..1, higher = faster).
    coeff: f32,
    /// Sample rate in Hz.
    sample_rate: f32,
    /// Smoothing time in seconds.
    smooth_time: f32,
}

impl ParamSmoother {
    /// Create a new parameter smoother.
    ///
    /// `smooth_time` is the time constant in seconds (time to reach ~63% of target).
    /// `initial_value` is the starting value.
    #[must_use]
    pub fn new(smooth_time: f32, sample_rate: f32, initial_value: f32) -> Self {
        let time = smooth_time.max(0.0);
        let coeff = if time > 0.0 {
            1.0 - (-1.0 / (time * sample_rate)).exp()
        } else {
            1.0 // instant
        };
        Self {
            current: initial_value,
            target: initial_value,
            coeff,
            sample_rate,
            smooth_time: time,
        }
    }

    /// Set a new target value. Non-finite values are ignored.
    pub fn set_target(&mut self, target: f32) {
        if target.is_finite() {
            self.target = target;
        }
    }

    /// Get the next smoothed value.
    #[inline]
    #[must_use]
    pub fn next_value(&mut self) -> f32 {
        self.current += self.coeff * (self.target - self.current);
        self.current = crate::flush_denormal(self.current);
        self.current
    }

    /// Check if the smoother has reached its target (within epsilon).
    #[inline]
    #[must_use]
    pub fn is_settled(&self) -> bool {
        (self.current - self.target).abs() < 1e-6
    }

    /// Snap immediately to the target value (skip smoothing).
    pub fn snap(&mut self) {
        self.current = self.target;
    }

    /// Set the smoothing time and recalculate coefficient.
    pub fn set_smooth_time(&mut self, time: f32) {
        self.smooth_time = time.max(0.0);
        self.coeff = if self.smooth_time > 0.0 {
            1.0 - (-1.0 / (self.smooth_time * self.sample_rate)).exp()
        } else {
            1.0
        };
    }

    /// Returns the current smoothed value without advancing.
    #[inline]
    #[must_use]
    pub fn current(&self) -> f32 {
        self.current
    }

    /// Returns the target value.
    #[inline]
    #[must_use]
    pub fn target(&self) -> f32 {
        self.target
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smoother_converges() {
        let mut s = ParamSmoother::new(0.01, 44100.0, 0.0);
        s.set_target(1.0);
        for _ in 0..10000 {
            let _ = s.next_value();
        }
        assert!(
            (s.current() - 1.0).abs() < 0.001,
            "should converge to target, got {}",
            s.current()
        );
    }

    #[test]
    fn test_smoother_instant() {
        let mut s = ParamSmoother::new(0.0, 44100.0, 0.0);
        s.set_target(1.0);
        let val = s.next_value();
        assert!(
            (val - 1.0).abs() < f32::EPSILON,
            "zero smooth_time should be instant"
        );
    }

    #[test]
    fn test_smoother_snap() {
        let mut s = ParamSmoother::new(1.0, 44100.0, 0.0);
        s.set_target(0.5);
        s.snap();
        assert!((s.current() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_serde_roundtrip() {
        let s = ParamSmoother::new(0.01, 44100.0, 0.5);
        let json = serde_json::to_string(&s).unwrap();
        let back: ParamSmoother = serde_json::from_str(&json).unwrap();
        assert!((s.current() - back.current()).abs() < f32::EPSILON);
    }
}
