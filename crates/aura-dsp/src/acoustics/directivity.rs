//! Source directivity patterns for monitor placement and spatial synthesis.
//!
//! Wraps goonj's analytical directivity patterns (omni / cardioid family /
//! figure-8) for use in naad. Given a source's front-axis vector and the
//! direction toward a listener / receiver, [`SourceDirectivity::gain`]
//! returns the linear attenuation factor that should be applied to the
//! signal before further propagation.
//!
//! Tabulated balloon data (CLF/CF2 measured directivity) is intentionally
//! out of scope here — load that via goonj directly if you need it; this
//! module exposes only the closed-form analytical patterns that map
//! cleanly onto naad's no-IO contract.

use serde::{Deserialize, Serialize};

use goonj::directivity::DirectivityPattern as GoonjPattern;
use hisab::Vec3;

/// An analytical directivity pattern.
///
/// Naad-side mirror of [`goonj::directivity::DirectivityPattern`] for the
/// closed-form variants. Re-defined locally so naad's public API does not
/// transitively expose goonj's enum (which may evolve independently in
/// future major versions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SourceDirectivity {
    /// Equal radiation in all directions.
    Omnidirectional,
    /// Cardioid: `gain = 0.5 × (1 + cos θ)`. Front-firing, null behind.
    Cardioid,
    /// Subcardioid (wide cardioid): `gain = 0.75 + 0.25 × cos θ`. Mostly omni with a forward bias.
    Subcardioid,
    /// Supercardioid: `gain = 0.37 + 0.63 × cos θ`. Tighter than cardioid, small rear lobe.
    Supercardioid,
    /// Figure-8 / dipole: `gain = |cos θ|`. Equal front + back, deep nulls at the sides.
    Figure8,
}

impl SourceDirectivity {
    /// Compute the linear directivity gain for a given direction relative to the source's front axis.
    ///
    /// `direction` is a unit vector from the source toward the evaluation point;
    /// `front` is the unit vector of the source's main radiation axis.
    /// Returns gain in linear scale where `1.0` is the on-axis reference.
    #[inline]
    #[must_use]
    pub fn gain(self, direction: Vec3, front: Vec3) -> f32 {
        self.to_goonj().gain(direction, front)
    }

    /// Evaluate the directivity gain for a polar angle `theta` (radians from front axis).
    ///
    /// Convenience for the common 2D case where you don't have full 3D
    /// vectors handy. `theta = 0` is on-axis, `theta = π` is directly behind.
    #[inline]
    #[must_use]
    pub fn gain_polar(self, theta: f32) -> f32 {
        let cos_theta = theta.cos().clamp(-1.0, 1.0);
        match self {
            Self::Omnidirectional => 1.0,
            Self::Cardioid => (0.5 * (1.0 + cos_theta)).max(0.0),
            Self::Subcardioid => 0.75 + 0.25 * cos_theta,
            Self::Supercardioid => (0.37 + 0.63 * cos_theta).max(0.0),
            Self::Figure8 => cos_theta.abs(),
        }
    }

    fn to_goonj(self) -> GoonjPattern {
        match self {
            Self::Omnidirectional => GoonjPattern::Omnidirectional,
            Self::Cardioid => GoonjPattern::Cardioid,
            Self::Subcardioid => GoonjPattern::Subcardioid,
            Self::Supercardioid => GoonjPattern::Supercardioid,
            Self::Figure8 => GoonjPattern::Figure8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_omnidirectional_is_unity_in_all_directions() {
        let front = Vec3::new(1.0, 0.0, 0.0);
        for dir in [
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        ] {
            let g = SourceDirectivity::Omnidirectional.gain(dir, front);
            assert!((g - 1.0).abs() < 1e-5, "omni dir {dir:?} → gain {g}");
        }
    }

    #[test]
    fn test_cardioid_null_at_rear() {
        // On-axis = 1.0, 180° = 0.0
        assert!((SourceDirectivity::Cardioid.gain_polar(0.0) - 1.0).abs() < 1e-5);
        assert!(SourceDirectivity::Cardioid.gain_polar(std::f32::consts::PI) < 1e-5);
    }

    #[test]
    fn test_figure8_nulls_at_sides() {
        // Figure-8: peaks at 0 and π, nulls at ±π/2.
        assert!((SourceDirectivity::Figure8.gain_polar(0.0) - 1.0).abs() < 1e-5);
        assert!((SourceDirectivity::Figure8.gain_polar(std::f32::consts::PI) - 1.0).abs() < 1e-5);
        assert!(SourceDirectivity::Figure8.gain_polar(std::f32::consts::FRAC_PI_2) < 1e-5);
    }

    #[test]
    fn test_supercardioid_has_small_rear_lobe() {
        // Supercardioid hits zero somewhere between π/2 and π.
        let g_rear = SourceDirectivity::Supercardioid.gain_polar(std::f32::consts::PI);
        // Should clip to 0 from `(0.37 + 0.63 * -1).max(0)` = `(-0.26).max(0)` = 0.
        assert!(g_rear < 1e-5);
        // But it's not always zero behind: at 2π/3, 0.37 + 0.63*(-0.5) = 0.055
        let g_oblique =
            SourceDirectivity::Supercardioid.gain_polar(2.0 * std::f32::consts::PI / 3.0);
        assert!(g_oblique > 0.0);
    }

    #[test]
    fn test_polar_matches_3d_for_axisymmetric() {
        let front = Vec3::new(1.0, 0.0, 0.0);
        for theta_deg in [0, 45, 90, 135, 180] {
            let theta = (theta_deg as f32).to_radians();
            let dir = Vec3::new(theta.cos(), theta.sin(), 0.0);
            for pat in [
                SourceDirectivity::Cardioid,
                SourceDirectivity::Subcardioid,
                SourceDirectivity::Supercardioid,
                SourceDirectivity::Figure8,
            ] {
                let g_3d = pat.gain(dir, front);
                let g_polar = pat.gain_polar(theta);
                assert!(
                    (g_3d - g_polar).abs() < 1e-4,
                    "{pat:?} @ {theta_deg}°: 3D={g_3d} polar={g_polar}"
                );
            }
        }
    }

    #[test]
    fn test_serde_roundtrip() {
        let pat = SourceDirectivity::Cardioid;
        let json = serde_json::to_string(&pat).unwrap();
        let back: SourceDirectivity = serde_json::from_str(&json).unwrap();
        assert_eq!(pat, back);
    }
}
