//! Stereo panning utilities.
//!
//! Provides equal-power and linear panning laws for stereo signal placement.

use serde::{Deserialize, Serialize};

/// Panning law used for stereo positioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PanLaw {
    /// Equal-power (constant-power) panning using sin/cos.
    /// Preserves perceived loudness across the stereo field.
    EqualPower,
    /// Linear panning. Simple but perceived loudness dips at center.
    Linear,
}

/// Stereo panning gains (left and right channel multipliers).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PanGains {
    /// Left channel gain.
    pub left: f32,
    /// Right channel gain.
    pub right: f32,
}

/// Compute stereo panning gains for a given pan position.
///
/// `pan` ranges from -1.0 (full left) to +1.0 (full right), with 0.0 = center.
#[inline]
#[must_use]
pub fn pan_gains(pan: f32, law: PanLaw) -> PanGains {
    let p = pan.clamp(-1.0, 1.0);
    // Map -1..+1 to 0..1
    let t = (p + 1.0) * 0.5;

    match law {
        PanLaw::EqualPower => {
            let angle = t * std::f32::consts::FRAC_PI_2;
            PanGains {
                left: angle.cos(),
                right: angle.sin(),
            }
        }
        PanLaw::Linear => PanGains {
            left: 1.0 - t,
            right: t,
        },
    }
}

/// Apply panning to a mono sample, returning (left, right).
#[inline]
#[must_use]
pub fn pan_mono(sample: f32, pan: f32, law: PanLaw) -> (f32, f32) {
    let g = pan_gains(pan, law);
    (sample * g.left, sample * g.right)
}

/// Stereo balance: attenuate one channel to shift the image.
///
/// `balance` ranges from -1.0 (left only) to +1.0 (right only).
#[inline]
#[must_use]
pub fn stereo_balance(left: f32, right: f32, balance: f32) -> (f32, f32) {
    let b = balance.clamp(-1.0, 1.0);
    if b < 0.0 {
        // Attenuate right
        (left, right * (1.0 + b))
    } else {
        // Attenuate left
        (left * (1.0 - b), right)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_center_equal_power() {
        let g = pan_gains(0.0, PanLaw::EqualPower);
        // At center, both channels should be equal (~0.707)
        assert!((g.left - g.right).abs() < 0.01);
        // Power should sum to ~1.0
        assert!((g.left * g.left + g.right * g.right - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_full_left() {
        let g = pan_gains(-1.0, PanLaw::EqualPower);
        assert!((g.left - 1.0).abs() < 0.01);
        assert!(g.right.abs() < 0.01);
    }

    #[test]
    fn test_full_right() {
        let g = pan_gains(1.0, PanLaw::EqualPower);
        assert!(g.left.abs() < 0.01);
        assert!((g.right - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_linear_center() {
        let g = pan_gains(0.0, PanLaw::Linear);
        assert!((g.left - 0.5).abs() < f32::EPSILON);
        assert!((g.right - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_pan_mono() {
        let (l, r) = pan_mono(1.0, -1.0, PanLaw::EqualPower);
        assert!((l - 1.0).abs() < 0.01);
        assert!(r.abs() < 0.01);
    }

    #[test]
    fn test_stereo_balance() {
        let (l, r) = stereo_balance(1.0, 1.0, -1.0);
        assert!((l - 1.0).abs() < f32::EPSILON);
        assert!(r.abs() < f32::EPSILON);
    }

    #[test]
    fn test_serde_roundtrip() {
        let g = pan_gains(0.3, PanLaw::EqualPower);
        let json = serde_json::to_string(&g).unwrap();
        let back: PanGains = serde_json::from_str(&json).unwrap();
        assert!((g.left - back.left).abs() < f32::EPSILON);
    }
}
