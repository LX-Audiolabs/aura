//! Ambisonics B-format encoding via goonj.
//!
//! Encodes a mono audio signal into first-order Ambisonics (B-format) using
//! spherical harmonic coefficients computed from the source direction.
//! Uses the same SN3D/ACN encoding as goonj's ambisonics module.

use serde::{Deserialize, Serialize};

/// A single B-format encoded sample (W, X, Y, Z channels).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BFormatSample {
    /// W channel — omnidirectional pressure (SN3D: gain = 1.0).
    pub w: f32,
    /// X channel — front/back (SN3D: gain = cos(az) * cos(el)).
    pub x: f32,
    /// Y channel — left/right (SN3D: gain = sin(az) * cos(el)).
    pub y: f32,
    /// Z channel — up/down (SN3D: gain = sin(el)).
    pub z: f32,
}

/// Ambisonics B-format encoder.
///
/// Encodes a mono input sample into first-order Ambisonics (W, X, Y, Z)
/// based on the source direction (azimuth and elevation).
///
/// Uses SN3D/ACN normalization matching goonj's [`goonj::ambisonics::encode_bformat`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmbisonicsEncoder {
    /// Azimuth angle in radians (0 = front, positive = right).
    azimuth: f32,
    /// Elevation angle in radians (0 = horizontal, positive = up).
    elevation: f32,
    /// Cached cosine of elevation.
    #[serde(skip)]
    cos_el: f32,
    /// Cached sine of elevation.
    #[serde(skip)]
    sin_el: f32,
    /// Cached cosine of azimuth.
    #[serde(skip)]
    cos_az: f32,
    /// Cached sine of azimuth.
    #[serde(skip)]
    sin_az: f32,
}

impl AmbisonicsEncoder {
    /// Create a new ambisonics encoder for the given source direction.
    #[must_use]
    pub fn new(azimuth: f32, elevation: f32) -> Self {
        let mut enc = Self {
            azimuth,
            elevation,
            cos_el: 0.0,
            sin_el: 0.0,
            cos_az: 0.0,
            sin_az: 0.0,
        };
        enc.update_cache();
        enc
    }

    /// Update the source direction.
    pub fn set_position(&mut self, azimuth: f32, elevation: f32) {
        self.azimuth = azimuth;
        self.elevation = elevation;
        self.update_cache();
    }

    /// Encode a mono input sample into B-format.
    ///
    /// W = input (omnidirectional)
    /// X = input * cos(az) * cos(el)
    /// Y = input * sin(az) * cos(el)
    /// Z = input * sin(el)
    #[inline]
    #[must_use]
    pub fn encode_sample(&self, input: f32) -> BFormatSample {
        // Ensure trig cache is valid (it may be zeroed after deserialization)
        let (cos_el, sin_el, cos_az, sin_az) = if self.cos_el == 0.0
            && self.sin_el == 0.0
            && self.cos_az == 0.0
            && self.sin_az == 0.0
            && (self.azimuth != 0.0 || self.elevation != 0.0)
        {
            // Fallback: compute on the fly (only after deser with non-zero angles)
            (
                self.elevation.cos(),
                self.elevation.sin(),
                self.azimuth.cos(),
                self.azimuth.sin(),
            )
        } else if self.cos_el == 0.0
            && self.sin_el == 0.0
            && self.cos_az == 0.0
            && self.sin_az == 0.0
        {
            // Zero angles: cos(0)=1, sin(0)=0
            (1.0, 0.0, 1.0, 0.0)
        } else {
            (self.cos_el, self.sin_el, self.cos_az, self.sin_az)
        };

        BFormatSample {
            w: input,
            x: input * cos_az * cos_el,
            y: input * sin_az * cos_el,
            z: input * sin_el,
        }
    }

    /// Current azimuth in radians.
    #[must_use]
    pub fn azimuth(&self) -> f32 {
        self.azimuth
    }

    /// Current elevation in radians.
    #[must_use]
    pub fn elevation(&self) -> f32 {
        self.elevation
    }

    /// Recompute cached trig values from current angles.
    fn update_cache(&mut self) {
        self.cos_el = self.elevation.cos();
        self.sin_el = self.elevation.sin();
        self.cos_az = self.azimuth.cos();
        self.sin_az = self.azimuth.sin();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_center_position_w_equals_input() {
        let enc = AmbisonicsEncoder::new(0.0, 0.0);
        let sample = enc.encode_sample(1.0);
        assert!(
            (sample.w - 1.0).abs() < f32::EPSILON,
            "W should equal input at center"
        );
        // At (0, 0): X = cos(0)*cos(0) = 1.0
        assert!(
            (sample.x - 1.0).abs() < 0.01,
            "X should be ~1.0 at front center, got {}",
            sample.x
        );
        // Y = sin(0)*cos(0) = 0.0
        assert!(
            sample.y.abs() < 0.01,
            "Y should be ~0 at center, got {}",
            sample.y
        );
        // Z = sin(0) = 0.0
        assert!(
            sample.z.abs() < 0.01,
            "Z should be ~0 at center, got {}",
            sample.z
        );
    }

    #[test]
    fn test_90_degrees_produces_expected_xy() {
        let enc = AmbisonicsEncoder::new(std::f32::consts::FRAC_PI_2, 0.0);
        let sample = enc.encode_sample(1.0);
        // At 90 deg: X = cos(pi/2)*cos(0) ~= 0, Y = sin(pi/2)*cos(0) ~= 1
        assert!(
            sample.x.abs() < 0.01,
            "X should be ~0 at 90deg, got {}",
            sample.x
        );
        assert!(
            (sample.y - 1.0).abs() < 0.01,
            "Y should be ~1 at 90deg, got {}",
            sample.y
        );
    }

    #[test]
    fn test_elevation_produces_z() {
        let enc = AmbisonicsEncoder::new(0.0, std::f32::consts::FRAC_PI_2);
        let sample = enc.encode_sample(1.0);
        // At elevation 90deg: Z = sin(pi/2) = 1.0
        assert!(
            (sample.z - 1.0).abs() < 0.01,
            "Z should be ~1 at 90deg elevation, got {}",
            sample.z
        );
    }

    #[test]
    fn test_set_position() {
        let mut enc = AmbisonicsEncoder::new(0.0, 0.0);
        enc.set_position(std::f32::consts::FRAC_PI_2, 0.0);
        let sample = enc.encode_sample(1.0);
        assert!(
            (sample.y - 1.0).abs() < 0.01,
            "Y should be ~1 after set_position to 90deg"
        );
    }

    #[test]
    fn test_zero_input() {
        let enc = AmbisonicsEncoder::new(0.5, 0.3);
        let sample = enc.encode_sample(0.0);
        assert_eq!(sample.w, 0.0);
        assert_eq!(sample.x, 0.0);
        assert_eq!(sample.y, 0.0);
        assert_eq!(sample.z, 0.0);
    }

    #[test]
    fn test_serde_roundtrip() {
        let enc = AmbisonicsEncoder::new(0.7, 0.3);
        let json = serde_json::to_string(&enc).unwrap();
        let back: AmbisonicsEncoder = serde_json::from_str(&json).unwrap();
        assert!((enc.azimuth - back.azimuth).abs() < f32::EPSILON);
        assert!((enc.elevation - back.elevation).abs() < f32::EPSILON);
        // Verify the deserialized encoder still works
        let sample = back.encode_sample(1.0);
        assert!(sample.w.is_finite());
        assert!(sample.x.is_finite());
    }

    #[test]
    fn test_serde_roundtrip_bformat_sample() {
        let sample = BFormatSample {
            w: 1.0,
            x: 0.5,
            y: -0.3,
            z: 0.1,
        };
        let json = serde_json::to_string(&sample).unwrap();
        let back: BFormatSample = serde_json::from_str(&json).unwrap();
        assert_eq!(sample, back);
    }
}
