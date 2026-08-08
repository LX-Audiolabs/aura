//! Room simulation reverb using goonj ray tracing.
//!
//! Generates an impulse response from a shoebox room model via the
//! image-source method and diffuse rain, then applies it as a convolution
//! reverb on incoming audio samples.

use serde::{Deserialize, Serialize};
use tracing::debug;

use goonj::impulse::{IrConfig, generate_ir};
use goonj::room::AcousticRoom;
use hisab::Vec3;

use crate::error::{NaadError, Result};

/// Configuration for a room simulation reverb.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomReverbConfig {
    /// Room length in metres (X axis).
    pub length: f32,
    /// Room width in metres (Z axis).
    pub width: f32,
    /// Room height in metres (Y axis).
    pub height: f32,
    /// Name of the wall material (e.g. `"concrete"`, `"carpet"`).
    pub wall_material_name: String,
    /// Source position `[x, y, z]` in metres.
    pub source_position: [f32; 3],
    /// Listener position `[x, y, z]` in metres.
    pub listener_position: [f32; 3],
    /// Sample rate in Hz.
    pub sample_rate: u32,
}

/// Room simulation reverb backed by a goonj impulse response.
#[derive(Debug, Clone)]
pub struct RoomReverb {
    /// The configuration used to build this reverb.
    config: RoomReverbConfig,
    /// Pre-computed impulse response.
    impulse_response: Vec<f32>,
    /// Ring-buffer of recent input samples (length == IR length).
    input_buffer: Vec<f32>,
    /// Current write position in the ring buffer.
    current_position: usize,
}

impl RoomReverb {
    /// Create a new room reverb from the given configuration.
    ///
    /// Builds a shoebox room in goonj, generates an impulse response via
    /// image-source + diffuse-rain, and stores it for sample-by-sample
    /// convolution.
    ///
    /// # Errors
    ///
    /// Returns [`NaadError::ComputationError`] if the wall material name is
    /// unknown or the room dimensions are invalid.
    pub fn new(config: RoomReverbConfig) -> Result<Self> {
        let material = super::material_by_name(&config.wall_material_name).ok_or_else(|| {
            NaadError::ComputationError {
                message: format!("unknown wall material: {}", config.wall_material_name),
            }
        })?;

        if config.length <= 0.0 || config.width <= 0.0 || config.height <= 0.0 {
            return Err(NaadError::ComputationError {
                message: "room dimensions must be positive".into(),
            });
        }

        let room = AcousticRoom::shoebox(config.length, config.width, config.height, material);

        let source = Vec3::new(
            config.source_position[0],
            config.source_position[1],
            config.source_position[2],
        );
        let listener = Vec3::new(
            config.listener_position[0],
            config.listener_position[1],
            config.listener_position[2],
        );

        let ir_config = IrConfig {
            sample_rate: config.sample_rate,
            max_order: 3,
            num_diffuse_rays: 2000,
            max_bounces: 30,
            max_time_seconds: 1.0,
            seed: 42,
        };

        let multiband = generate_ir(source, listener, &room, &ir_config);
        let broadband = multiband.to_broadband();
        let ir = broadband.samples;

        let ir_len = ir.len().max(1);
        debug!(
            ir_len,
            config.length, config.width, config.height, "room reverb created"
        );
        Ok(Self {
            config,
            impulse_response: ir,
            input_buffer: vec![0.0; ir_len],
            current_position: 0,
        })
    }

    /// Process a single audio sample through the room reverb.
    ///
    /// Uses direct convolution with the stored impulse response.
    #[inline]
    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let ir_len = self.impulse_response.len();
        if ir_len == 0 {
            return input;
        }

        // Write input into ring buffer
        self.input_buffer[self.current_position] = input;

        // Direct convolution: output = sum(input_buffer[pos - k] * ir[k])
        let mut output = 0.0_f32;
        for (k, &h) in self.impulse_response.iter().enumerate() {
            let idx = (self.current_position + ir_len - k) % ir_len;
            output += self.input_buffer[idx] * h;
        }

        self.current_position = (self.current_position + 1) % ir_len;
        output
    }

    /// Returns a reference to the impulse response.
    #[must_use]
    pub fn impulse_response(&self) -> &[f32] {
        &self.impulse_response
    }

    /// Returns a reference to the configuration.
    #[must_use]
    pub fn config(&self) -> &RoomReverbConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> RoomReverbConfig {
        RoomReverbConfig {
            length: 8.0,
            width: 6.0,
            height: 3.0,
            wall_material_name: "concrete".into(),
            source_position: [2.0, 1.5, 3.0],
            listener_position: [6.0, 1.5, 3.0],
            sample_rate: 48000,
        }
    }

    #[test]
    fn test_room_reverb_creates() {
        let reverb = RoomReverb::new(test_config());
        assert!(reverb.is_ok(), "should create room reverb");
    }

    #[test]
    fn test_room_reverb_produces_finite_output() {
        let mut reverb = RoomReverb::new(test_config()).unwrap();
        // Feed an impulse
        let out = reverb.process_sample(1.0);
        assert!(out.is_finite(), "output should be finite");
        // Feed silence
        for _ in 0..1000 {
            let s = reverb.process_sample(0.0);
            assert!(s.is_finite(), "tail should be finite");
        }
    }

    #[test]
    fn test_unknown_material_errors() {
        let mut cfg = test_config();
        cfg.wall_material_name = "unobtanium".into();
        let result = RoomReverb::new(cfg);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_dimensions_error() {
        let mut cfg = test_config();
        cfg.length = -1.0;
        let result = RoomReverb::new(cfg);
        assert!(result.is_err());
    }

    #[test]
    fn test_serde_roundtrip_config() {
        let config = test_config();
        let json = serde_json::to_string(&config).unwrap();
        let back: RoomReverbConfig = serde_json::from_str(&json).unwrap();
        assert!((config.length - back.length).abs() < f32::EPSILON);
        assert_eq!(config.wall_material_name, back.wall_material_name);
    }
}
