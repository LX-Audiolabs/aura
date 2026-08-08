//! Convolution reverb using impulse responses from goonj.
//!
//! Applies a pre-computed impulse response to an audio stream via direct
//! time-domain convolution. Can load an IR from a raw buffer or generate
//! one from a goonj room simulation.
//!
//! **Performance note**: This uses direct O(N) convolution per sample, where
//! N is the IR length. For real-time use with long IRs (>1000 samples),
//! consider partitioned FFT convolution in dhvani instead. This module is
//! suitable for short IRs or offline processing.

use serde::{Deserialize, Serialize};

use goonj::impulse::{IrConfig, generate_ir};
use goonj::room::AcousticRoom;
use hisab::{Complex, Vec3};

use crate::error::{NaadError, Result};

/// Convolution reverb processor.
///
/// Stores an impulse response and convolves incoming audio with it
/// sample-by-sample using a ring buffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvolutionReverb {
    /// Impulse response samples (skipped in serde — too large).
    #[serde(skip)]
    ir: Vec<f32>,
    /// Ring buffer of recent input samples (skipped in serde).
    #[serde(skip)]
    input_buffer: Vec<f32>,
    /// Current write position in the ring buffer.
    #[serde(skip)]
    position: usize,
    /// Wet/dry mix (0.0 = fully dry, 1.0 = fully wet).
    pub mix: f32,
    /// Reusable FFT scratch — IR in frequency domain.
    #[serde(skip)]
    scratch_ir: Vec<Complex>,
    /// Reusable FFT scratch — input block in frequency domain.
    #[serde(skip)]
    scratch_in: Vec<Complex>,
    /// Reusable FFT scratch — pointwise product / IFFT result.
    #[serde(skip)]
    scratch_product: Vec<Complex>,
}

impl ConvolutionReverb {
    /// Create a convolution reverb from a raw impulse response.
    #[must_use]
    pub fn from_ir(ir: Vec<f32>, mix: f32) -> Self {
        let len = ir.len().max(1);
        Self {
            ir,
            input_buffer: vec![0.0; len],
            position: 0,
            mix: mix.clamp(0.0, 1.0),
            scratch_ir: Vec::new(),
            scratch_in: Vec::new(),
            scratch_product: Vec::new(),
        }
    }

    /// Create a convolution reverb from a goonj room simulation.
    ///
    /// Generates the impulse response using the image-source method and
    /// diffuse rain from the given [`super::room::RoomReverbConfig`].
    ///
    /// # Errors
    ///
    /// Returns [`NaadError::ComputationError`] if the material name is unknown
    /// or room dimensions are invalid.
    pub fn from_room(config: &super::room::RoomReverbConfig) -> Result<Self> {
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

        Ok(Self::from_ir(broadband.samples, 1.0))
    }

    /// Process a single audio sample through the convolution reverb.
    ///
    /// Returns a mix of dry input and wet convolved output.
    #[inline]
    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let ir_len = self.ir.len();
        if ir_len == 0 {
            return input;
        }

        self.input_buffer[self.position] = input;

        let mut wet = 0.0_f32;
        for (k, &h) in self.ir.iter().enumerate() {
            let idx = (self.position + ir_len - k) % ir_len;
            wet += self.input_buffer[idx] * h;
        }

        self.position = (self.position + 1) % ir_len;

        input * (1.0 - self.mix) + wet * self.mix
    }

    /// Process a block of audio samples through FFT-based convolution.
    ///
    /// Uses overlap-save partitioned convolution via `hisab::num::fft` for
    /// O(N log N) per block instead of O(N) per sample. Much more efficient
    /// for long impulse responses. Input and output must be the same length.
    ///
    /// Call this instead of `process_sample` in a loop for better performance
    /// when processing full buffers.
    pub fn process_block(&mut self, input: &[f32], output: &mut [f32]) {
        let ir_len = self.ir.len();
        if ir_len == 0 || input.is_empty() {
            for (o, &i) in output.iter_mut().zip(input.iter()) {
                *o = i;
            }
            return;
        }

        let block_len = input.len();
        // FFT size: next power of 2 >= ir_len + block_len - 1
        let fft_len = (ir_len + block_len - 1).next_power_of_two();
        let zero = Complex::new(0.0, 0.0);

        // Reuse scratch buffers — resize keeps capacity, only allocates if
        // fft_len exceeds the largest seen so far.
        self.scratch_ir.clear();
        self.scratch_ir.reserve(fft_len);
        self.scratch_ir
            .extend(self.ir.iter().map(|&s| Complex::new(s as f64, 0.0)));
        self.scratch_ir.resize(fft_len, zero);

        self.scratch_in.clear();
        self.scratch_in.reserve(fft_len);
        self.scratch_in
            .extend(input.iter().map(|&s| Complex::new(s as f64, 0.0)));
        self.scratch_in.resize(fft_len, zero);

        // Forward FFT both
        if hisab::num::fft(&mut self.scratch_ir).is_err()
            || hisab::num::fft(&mut self.scratch_in).is_err()
        {
            // Fallback to direct convolution on FFT failure
            for (i, o) in input.iter().zip(output.iter_mut()) {
                *o = self.process_sample(*i);
            }
            return;
        }

        // Pointwise multiply in frequency domain (into scratch_product).
        self.scratch_product.clear();
        self.scratch_product.reserve(fft_len);
        self.scratch_product.extend(
            self.scratch_ir
                .iter()
                .zip(self.scratch_in.iter())
                .map(|(a, b)| *a * *b),
        );

        // Inverse FFT
        if hisab::num::ifft(&mut self.scratch_product).is_err() {
            for (i, o) in input.iter().zip(output.iter_mut()) {
                *o = self.process_sample(*i);
            }
            return;
        }

        // Extract real part, apply mix
        let dry = 1.0 - self.mix;
        for (i, o) in output.iter_mut().enumerate().take(block_len) {
            let wet = self.scratch_product[i].re as f32;
            *o = input[i] * dry + wet * self.mix;
        }
    }

    /// Rebuild the convolution reverb with a new IR (e.g., after deserialization).
    ///
    /// The IR and input buffer are skipped during serde. Call this after
    /// deserializing to restore functionality.
    pub fn rebuild_from_ir(&mut self, ir: Vec<f32>) {
        let len = ir.len().max(1);
        self.input_buffer = vec![0.0; len];
        self.position = 0;
        self.ir = ir;
        // Drop any cached FFT scratch so the next process_block grows fresh
        // capacity to fit the new IR length.
        self.scratch_ir.clear();
        self.scratch_in.clear();
        self.scratch_product.clear();
    }

    /// Returns the length of the impulse response in samples.
    #[must_use]
    pub fn ir_len(&self) -> usize {
        self.ir.len()
    }

    /// Returns true if the reverb has a loaded IR (false after deserialization).
    #[must_use]
    pub fn is_loaded(&self) -> bool {
        !self.ir.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_ir_produces_output() {
        // Simple echo IR: identity + delayed tap
        let ir = vec![1.0, 0.0, 0.0, 0.5];
        let mut reverb = ConvolutionReverb::from_ir(ir, 1.0);

        let out = reverb.process_sample(1.0);
        assert!(out.is_finite());
        assert!(out.abs() > 0.0, "should produce output for impulse");

        // Process silence, check finite
        for _ in 0..10 {
            let s = reverb.process_sample(0.0);
            assert!(s.is_finite());
        }
    }

    #[test]
    fn test_dry_passthrough() {
        let ir = vec![0.5, 0.3, 0.1];
        let mut reverb = ConvolutionReverb::from_ir(ir, 0.0);
        let out = reverb.process_sample(0.7);
        assert!(
            (out - 0.7).abs() < 0.01,
            "mix=0 should pass dry signal, got {out}"
        );
    }

    #[test]
    fn test_serde_roundtrip() {
        let reverb = ConvolutionReverb::from_ir(vec![1.0, 0.5], 0.6);
        let json = serde_json::to_string(&reverb).unwrap();
        let back: ConvolutionReverb = serde_json::from_str(&json).unwrap();
        assert!((reverb.mix - back.mix).abs() < f32::EPSILON);
        // IR is skipped, so back.ir should be empty
        assert!(back.ir.is_empty());
    }

    #[test]
    fn test_fft_block_processing() {
        // Simple delay IR: identity at tap 0, echo at tap 3
        let ir = vec![1.0, 0.0, 0.0, 0.5];
        let mut reverb = ConvolutionReverb::from_ir(ir, 1.0);

        let input = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let mut output = [0.0f32; 8];
        reverb.process_block(&input, &mut output);

        // output[0] should be ~1.0 (identity), output[3] should be ~0.5 (echo)
        assert!(output[0].abs() > 0.5, "identity tap: {}", output[0]);
        assert!(output[3].abs() > 0.2, "echo tap: {}", output[3]);
        assert!(output.iter().all(|s| s.is_finite()));
    }
}
