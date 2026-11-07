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
    let t = f32::midpoint(p, 1.0);

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

/// Pan position in `[-1, 1]` for unison voice `index` of `num_voices`.
///
/// `spread` 0 = all center; 1 = full left..right by index. Equal-power or
/// linear gains via the existing [`pan_gains`].
#[inline]
#[must_use]
pub fn unison_voice_pan_gains(
    index: usize,
    num_voices: usize,
    spread: f32,
    law: PanLaw,
) -> PanGains {
    if num_voices <= 1 {
        return pan_gains(0.0, law);
    }
    let spread = if spread.is_finite() {
        spread.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let index = index.min(num_voices - 1);
    let t = index as f32 / (num_voices - 1) as f32;
    let pan = (t * 2.0 - 1.0) * spread;
    pan_gains(pan, law)
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

    #[test]
    fn unison_one_voice_is_center() {
        let g = unison_voice_pan_gains(0, 1, 1.0, PanLaw::EqualPower);
        assert!((g.left - g.right).abs() < 0.01);
    }

    #[test]
    fn unison_edges_full_spread() {
        let n = 4;
        let left = unison_voice_pan_gains(0, n, 1.0, PanLaw::Linear);
        let right = unison_voice_pan_gains(n - 1, n, 1.0, PanLaw::Linear);
        assert!(left.left > 0.99 && left.right < 0.01);
        assert!(right.right > 0.99 && right.left < 0.01);
    }

    #[test]
    fn unison_zero_spread_center() {
        let g = unison_voice_pan_gains(0, 8, 0.0, PanLaw::EqualPower);
        assert!((g.left - g.right).abs() < 0.01);
    }

    #[test]
    fn unison_non_finite_spread_center() {
        let g = unison_voice_pan_gains(0, 4, f32::NAN, PanLaw::EqualPower);
        assert!((g.left - g.right).abs() < 0.01);
    }

    #[test]
    fn unison_index_past_end_clamps_to_last() {
        let n = 4;
        let last = unison_voice_pan_gains(n - 1, n, 1.0, PanLaw::Linear);
        let oob = unison_voice_pan_gains(100, n, 1.0, PanLaw::Linear);
        assert!((last.left - oob.left).abs() < 1e-6);
        assert!((last.right - oob.right).abs() < 1e-6);
    }
}
