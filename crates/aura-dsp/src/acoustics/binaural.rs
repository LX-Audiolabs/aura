//! Binaural spatialization via goonj HRTF processing.
//!
//! Generates a binaural impulse response pair (left/right ear) from a
//! goonj room simulation with HRTF data, then convolves mono input into
//! spatialized stereo output.

use serde::{Deserialize, Serialize};
use tracing::debug;

use goonj::binaural::{HrtfDataset, HrtfPair, generate_binaural_ir};
use goonj::impulse::IrConfig;
use goonj::material::AcousticMaterial;
use goonj::room::AcousticRoom;
use hisab::Vec3;

use crate::error::{NaadError, Result};

/// Binaural HRTF processor for spatialization of mono audio.
///
/// Convolves mono input with left and right ear impulse responses to
/// produce headphone-ready stereo output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinauralProcessor {
    /// Left ear impulse response (skipped in serde).
    #[serde(skip)]
    left_ir: Vec<f32>,
    /// Right ear impulse response (skipped in serde).
    #[serde(skip)]
    right_ir: Vec<f32>,
    /// Ring buffer for left-ear convolution (skipped in serde).
    #[serde(skip)]
    left_buffer: Vec<f32>,
    /// Ring buffer for right-ear convolution (skipped in serde).
    #[serde(skip)]
    right_buffer: Vec<f32>,
    /// Current write position in the ring buffers.
    #[serde(skip)]
    position: usize,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Azimuth angle used to generate the IRs (radians).
    pub azimuth: f32,
    /// Elevation angle used to generate the IRs (radians).
    pub elevation: f32,
}

impl BinauralProcessor {
    /// Create a binaural processor for the given source direction.
    ///
    /// Generates left/right ear impulse responses using a simple room model
    /// with a minimal HRTF dataset derived from the requested azimuth and
    /// elevation angles.
    ///
    /// # Errors
    ///
    /// Returns [`NaadError::ComputationError`] if the binaural IR generation
    /// fails.
    pub fn new(azimuth: f32, elevation: f32, sample_rate: u32) -> Result<Self> {
        if sample_rate == 0 {
            return Err(NaadError::ComputationError {
                message: "sample rate must be > 0".into(),
            });
        }

        // Build a small room for the binaural simulation
        let room = AcousticRoom::shoebox(6.0, 4.0, 3.0, AcousticMaterial::concrete());
        let source = Vec3::new(2.0, 1.5, 2.0);
        let listener = Vec3::new(4.0, 1.5, 2.0);

        // Build a minimal HRTF dataset with direction-dependent ear differences
        let hrtf = build_minimal_hrtf(azimuth, elevation, sample_rate);

        let ir_config = IrConfig {
            sample_rate,
            max_order: 2,
            num_diffuse_rays: 500,
            max_bounces: 20,
            max_time_seconds: 0.3,
            seed: 42,
        };

        let binaural = generate_binaural_ir(source, listener, &room, &hrtf, &ir_config);

        let left_ir = binaural.left;
        let right_ir = binaural.right;
        let ir_len = left_ir.len().max(right_ir.len()).max(1);

        debug!(
            azimuth,
            elevation, ir_len, sample_rate, "binaural processor created"
        );
        Ok(Self {
            left_ir,
            right_ir,
            left_buffer: vec![0.0; ir_len],
            right_buffer: vec![0.0; ir_len],
            position: 0,
            sample_rate,
            azimuth,
            elevation,
        })
    }

    /// Process a mono input sample into stereo (left, right) output.
    #[inline]
    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> (f32, f32) {
        let left_len = self.left_ir.len();
        let right_len = self.right_ir.len();

        if left_len == 0 && right_len == 0 {
            return (input, input);
        }

        let buf_len = self.left_buffer.len();
        if buf_len == 0 {
            return (input, input);
        }

        self.left_buffer[self.position] = input;
        self.right_buffer[self.position] = input;

        let mut left_out = 0.0_f32;
        for (k, &h) in self.left_ir.iter().enumerate() {
            let idx = (self.position + buf_len - k) % buf_len;
            left_out += self.left_buffer[idx] * h;
        }

        let mut right_out = 0.0_f32;
        for (k, &h) in self.right_ir.iter().enumerate() {
            let idx = (self.position + buf_len - k) % buf_len;
            right_out += self.right_buffer[idx] * h;
        }

        self.position = (self.position + 1) % buf_len;

        (left_out, right_out)
    }

    /// Rebuild the binaural processor after deserialization.
    ///
    /// IRs and buffers are skipped during serde. Call this to regenerate
    /// them from the stored azimuth/elevation/sample_rate.
    ///
    /// # Errors
    ///
    /// Returns error if IR generation fails.
    pub fn rebuild(&mut self) -> Result<()> {
        let new = Self::new(self.azimuth, self.elevation, self.sample_rate)?;
        self.left_ir = new.left_ir;
        self.right_ir = new.right_ir;
        self.left_buffer = new.left_buffer;
        self.right_buffer = new.right_buffer;
        self.position = 0;
        Ok(())
    }

    /// Returns true if the processor has loaded IRs (false after deserialization).
    #[must_use]
    pub fn is_loaded(&self) -> bool {
        !self.left_ir.is_empty()
    }
}

/// Build a minimal HRTF dataset for binaural processing.
///
/// Creates synthetic HRTFs at the requested direction plus front/back/side
/// reference directions. The synthetic HRIRs approximate interaural time
/// and level differences based on azimuth.
fn build_minimal_hrtf(azimuth: f32, elevation: f32, sample_rate: u32) -> HrtfDataset {
    let hrir_len = 32;

    // Helper: generate a simple decaying impulse with level/delay difference
    let make_pair = |az: f32, el: f32| -> HrtfPair {
        let mut left = vec![0.0_f32; hrir_len];
        let mut right = vec![0.0_f32; hrir_len];

        // Interaural level difference based on azimuth
        let left_gain = (1.0 - az.sin() * 0.5).max(0.1);
        let right_gain = (1.0 + az.sin() * 0.5).max(0.1);

        // Simple decaying impulse
        for i in 0..hrir_len {
            let decay = (-3.0 * i as f32 / hrir_len as f32).exp();
            left[i] = left_gain * decay;
            right[i] = right_gain * decay;
        }

        HrtfPair {
            azimuth: az,
            elevation: el,
            left,
            right,
        }
    };

    let pairs = vec![
        make_pair(0.0, 0.0),                          // front
        make_pair(std::f32::consts::FRAC_PI_2, 0.0),  // right
        make_pair(-std::f32::consts::FRAC_PI_2, 0.0), // left
        make_pair(std::f32::consts::PI, 0.0),         // back
        make_pair(azimuth, elevation),                // requested direction
    ];

    HrtfDataset::from_pairs(pairs, sample_rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binaural_creates() {
        let proc = BinauralProcessor::new(0.5, 0.0, 48000);
        assert!(proc.is_ok(), "should create binaural processor");
    }

    #[test]
    fn test_binaural_produces_stereo() {
        let mut proc = BinauralProcessor::new(0.5, 0.0, 48000).unwrap();
        let (l, r) = proc.process_sample(1.0);
        assert!(l.is_finite());
        assert!(r.is_finite());
    }

    #[test]
    fn test_binaural_stereo_differs_at_side() {
        // At 90 degrees azimuth, left and right should differ
        let mut proc = BinauralProcessor::new(std::f32::consts::FRAC_PI_2, 0.0, 48000).unwrap();

        let mut diff_found = false;
        // Feed impulse and check all outputs including first sample
        for i in 0..1000 {
            let input = if i == 0 { 1.0 } else { 0.0 };
            let (l, r) = proc.process_sample(input);
            if (l - r).abs() > 1e-6 {
                diff_found = true;
                break;
            }
        }
        assert!(diff_found, "stereo output should differ for side source");
    }

    #[test]
    fn test_zero_sample_rate_errors() {
        let result = BinauralProcessor::new(0.0, 0.0, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_serde_roundtrip() {
        let proc = BinauralProcessor::new(0.3, 0.1, 48000).unwrap();
        let json = serde_json::to_string(&proc).unwrap();
        let back: BinauralProcessor = serde_json::from_str(&json).unwrap();
        assert!((proc.azimuth - back.azimuth).abs() < f32::EPSILON);
        assert_eq!(proc.sample_rate, back.sample_rate);
        // IRs are skipped
        assert!(back.left_ir.is_empty());
    }
}
