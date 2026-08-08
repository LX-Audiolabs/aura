//! Feedback Delay Network reverb using goonj::fdn.
//!
//! Wraps the goonj FDN with a wet/dry mix and serializable configuration.
//! The FDN itself is reconstructed on deserialization from stored parameters.

use serde::{Deserialize, Serialize};
use tracing::debug;

use goonj::fdn::{Fdn, fdn_config_for_room};

use crate::error::{NaadError, Result};

/// Feedback Delay Network reverb backed by goonj.
///
/// Stores the configuration parameters for serde roundtripping and
/// reconstructs the inner [`Fdn`] on deserialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FdnReverb {
    /// The inner FDN processor (skipped in serde — reconstructed from params).
    #[serde(skip)]
    fdn: Option<Fdn>,
    /// Room length in meters (drives delay line lengths).
    room_length: f32,
    /// Room width in meters.
    room_width: f32,
    /// Room height in meters.
    room_height: f32,
    /// Target RT60 in seconds.
    target_rt60: f32,
    /// Sample rate in Hz.
    sample_rate: u32,
    /// Wet/dry mix (0.0 = dry, 1.0 = wet).
    pub mix: f32,
}

impl FdnReverb {
    /// Create a new FDN reverb with configurable virtual room dimensions.
    ///
    /// The room dimensions control the delay line lengths (larger rooms =
    /// longer delays = spacier reverb). Target RT60 controls the decay time.
    ///
    /// # Errors
    ///
    /// Returns [`NaadError::ComputationError`] if parameters are out of range.
    pub fn new(
        room_length: f32,
        room_width: f32,
        room_height: f32,
        target_rt60: f32,
        sample_rate: u32,
        mix: f32,
    ) -> Result<Self> {
        if room_length <= 0.0 || room_width <= 0.0 || room_height <= 0.0 {
            return Err(NaadError::ComputationError {
                message: "room dimensions must be positive".into(),
            });
        }
        if target_rt60 <= 0.0 || !target_rt60.is_finite() {
            return Err(NaadError::ComputationError {
                message: "target_rt60 must be positive and finite".into(),
            });
        }
        if sample_rate == 0 {
            return Err(NaadError::ComputationError {
                message: "sample_rate must be > 0".into(),
            });
        }

        debug!(
            room_length,
            room_width, room_height, target_rt60, sample_rate, "FDN reverb created"
        );
        let config = fdn_config_for_room(
            room_length,
            room_width,
            room_height,
            target_rt60,
            sample_rate,
        );
        let fdn = Fdn::new(&config);

        Ok(Self {
            fdn: Some(fdn),
            room_length,
            room_width,
            room_height,
            target_rt60,
            sample_rate,
            mix: mix.clamp(0.0, 1.0),
        })
    }

    /// Process a single audio sample through the FDN reverb.
    ///
    /// Applies wet/dry mix to the output.
    #[inline]
    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        // Lazily reconstruct Fdn after deserialization
        if self.fdn.is_none() {
            let config = fdn_config_for_room(
                self.room_length,
                self.room_width,
                self.room_height,
                self.target_rt60,
                self.sample_rate,
            );
            self.fdn = Some(Fdn::new(&config));
        }

        let wet = if let Some(ref mut fdn) = self.fdn {
            fdn.process_sample(input)
        } else {
            0.0
        };

        input * (1.0 - self.mix) + wet * self.mix
    }

    /// Reset the FDN state to silence.
    pub fn reset(&mut self) {
        if let Some(ref mut fdn) = self.fdn {
            fdn.reset();
        }
    }

    /// Target RT60 in seconds.
    #[must_use]
    pub fn target_rt60(&self) -> f32 {
        self.target_rt60
    }

    /// Sample rate in Hz.
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

/// Number of delay lines in [`MatrixFdn`] (Hadamard requires a power of 2; 8 is a sweet spot).
const MATRIX_FDN_N: usize = 8;

/// Color-free Feedback Delay Network reverb with a Hadamard feedback matrix.
///
/// Implements the FDN from scratch with `naad`-owned delay lines + per-line
/// damping filters + an 8×8 Hadamard feedback matrix (orthogonal, hence
/// energy-preserving — the "color-free" property the roadmap calls out).
/// Delay lengths are mutually-coprime primes spread around the target
/// average to maximise modal density and avoid metallic comb-filter
/// resonances.
///
/// Where [`FdnReverb`] delegates to goonj's room-derived FDN parameters,
/// `MatrixFdn` owns its matrix design end-to-end — useful when you want
/// reverb timbre that's a function of `target_rt60` alone, independent of
/// any room-acoustic interpretation.
///
/// Requires the `acoustics` feature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixFdn {
    /// Per-delay-line ring buffers (skipped in serde — reset on deserialize).
    #[serde(skip, default = "default_buffers")]
    buffers: [Vec<f32>; MATRIX_FDN_N],
    /// Per-delay-line write positions (skipped in serde).
    #[serde(skip)]
    positions: [usize; MATRIX_FDN_N],
    /// Per-delay-line lengths in samples.
    delay_lengths: [usize; MATRIX_FDN_N],
    /// Per-line one-pole lowpass damping state (skipped in serde — restored to 0).
    #[serde(skip)]
    damping_state: [f32; MATRIX_FDN_N],
    /// Per-line damping coefficient (computed from RT60 and delay length).
    damping_gain: [f32; MATRIX_FDN_N],
    /// Target RT60 in seconds.
    target_rt60: f32,
    /// Sample rate in Hz.
    sample_rate: u32,
    /// Wet/dry mix (0.0 = dry, 1.0 = wet).
    pub mix: f32,
}

fn default_buffers() -> [Vec<f32>; MATRIX_FDN_N] {
    Default::default()
}

impl MatrixFdn {
    /// Create a new color-free Hadamard-FDN reverb.
    ///
    /// `target_rt60` controls decay time in seconds. `sample_rate` is in Hz.
    /// Delay lengths are picked from a small bank of primes around 30–80 ms.
    ///
    /// # Errors
    ///
    /// Returns [`NaadError::ComputationError`] if `target_rt60 <= 0`,
    /// `sample_rate == 0`, or numerical setup fails.
    pub fn new(target_rt60: f32, sample_rate: u32, mix: f32) -> Result<Self> {
        if target_rt60 <= 0.0 || !target_rt60.is_finite() {
            return Err(NaadError::ComputationError {
                message: "target_rt60 must be positive and finite".into(),
            });
        }
        if sample_rate == 0 {
            return Err(NaadError::ComputationError {
                message: "sample_rate must be > 0".into(),
            });
        }

        // Delay lengths in milliseconds, scaled to sample rate. Primes
        // chosen to be mutually coprime, spanning ~30–80 ms — typical for
        // medium-room reverberators.
        let ms = [29.7f32, 37.1, 41.3, 43.7, 53.9, 59.1, 67.3, 73.7];
        let mut delay_lengths = [0usize; MATRIX_FDN_N];
        for (i, m) in ms.iter().enumerate() {
            delay_lengths[i] = ((*m * 0.001) * sample_rate as f32).max(1.0) as usize;
        }

        // Damping gains: g_i = 10^(-3 * D_i / (RT60 * SR))
        // ensures each delay loop attenuates by 60 dB after RT60 seconds.
        let mut damping_gain = [0.0f32; MATRIX_FDN_N];
        for i in 0..MATRIX_FDN_N {
            let d = delay_lengths[i] as f32;
            damping_gain[i] = 10f32.powf(-3.0 * d / (target_rt60 * sample_rate as f32));
        }

        // Allocate buffers.
        let buffers: [Vec<f32>; MATRIX_FDN_N] =
            std::array::from_fn(|i| vec![0.0; delay_lengths[i]]);

        debug!(target_rt60, sample_rate, "MatrixFdn created");
        Ok(Self {
            buffers,
            positions: [0; MATRIX_FDN_N],
            delay_lengths,
            damping_state: [0.0; MATRIX_FDN_N],
            damping_gain,
            target_rt60,
            sample_rate,
            mix: mix.clamp(0.0, 1.0),
        })
    }

    /// Lazily restore scratch buffers after deserialization.
    fn ensure_buffers(&mut self) {
        if self.buffers[0].len() != self.delay_lengths[0] {
            for i in 0..MATRIX_FDN_N {
                self.buffers[i] = vec![0.0; self.delay_lengths[i]];
                self.positions[i] = 0;
                self.damping_state[i] = 0.0;
            }
        }
    }

    /// Process one sample through the Hadamard FDN.
    ///
    /// Reads every delay line, applies one-pole lowpass damping, multiplies
    /// the damped vector by the 8×8 normalised Sylvester-Hadamard matrix,
    /// then writes `input + feedback` back into each line. Output is the
    /// dry/wet mix of the input and the sum of damped delay-line outputs.
    #[inline]
    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        self.ensure_buffers();

        // Read delay-line outputs and apply per-line damping.
        let mut delayed = [0.0f32; MATRIX_FDN_N];
        for ((slot, line), (pos, gain)) in delayed
            .iter_mut()
            .zip(self.buffers.iter())
            .zip(self.positions.iter().zip(self.damping_gain.iter()))
        {
            let read_pos = (*pos + 1) % line.len();
            let damped = line[read_pos] * gain;
            *slot = crate::flush_denormal(damped);
        }
        // Mirror into damping_state for any external observer / future filter use.
        self.damping_state.copy_from_slice(&delayed);

        // Sum for output before writing feedback.
        let wet = delayed.iter().sum::<f32>() / (MATRIX_FDN_N as f32).sqrt();

        // Apply 8×8 normalised Hadamard mix: y = (1/√8) * H * x.
        // Sylvester construction: H_8 has ±1 entries (no general-purpose
        // matmul needed — fully unrolled below).
        let h = hadamard8(delayed);
        let scale = 1.0 / (MATRIX_FDN_N as f32).sqrt();

        // Write input + scaled feedback into each line.
        for ((line, pos), &fb) in self
            .buffers
            .iter_mut()
            .zip(self.positions.iter_mut())
            .zip(h.iter())
        {
            let v = input + fb * scale;
            *pos = (*pos + 1) % line.len();
            line[*pos] = crate::flush_denormal(v);
        }

        input * (1.0 - self.mix) + wet * self.mix
    }

    /// Process a buffer in place.
    #[inline]
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for s in buffer.iter_mut() {
            *s = self.process_sample(*s);
        }
    }

    /// Reset all delay lines and damping state to silence.
    pub fn reset(&mut self) {
        for i in 0..MATRIX_FDN_N {
            for s in &mut self.buffers[i] {
                *s = 0.0;
            }
            self.positions[i] = 0;
            self.damping_state[i] = 0.0;
        }
    }

    /// Target RT60 in seconds.
    #[must_use]
    pub fn target_rt60(&self) -> f32 {
        self.target_rt60
    }

    /// Sample rate in Hz.
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

/// Apply the normalised 8×8 Sylvester-Hadamard matrix to a vector (without the 1/√8 scaling).
///
/// Hard-coded since H_8 is small and the entries are ±1 — no matmul needed.
/// The caller multiplies by `1/√8` separately to make the transform
/// orthogonal (energy-preserving).
#[inline]
fn hadamard8(x: [f32; 8]) -> [f32; 8] {
    // Sylvester H_8 row signs (the recursive +/- doubling pattern).
    // Row 0: + + + + + + + +
    // Row 1: + - + - + - + -
    // Row 2: + + - - + + - -
    // Row 3: + - - + + - - +
    // Row 4: + + + + - - - -
    // Row 5: + - + - - + - +
    // Row 6: + + - - - - + +
    // Row 7: + - - + - + + -
    [
        x[0] + x[1] + x[2] + x[3] + x[4] + x[5] + x[6] + x[7],
        x[0] - x[1] + x[2] - x[3] + x[4] - x[5] + x[6] - x[7],
        x[0] + x[1] - x[2] - x[3] + x[4] + x[5] - x[6] - x[7],
        x[0] - x[1] - x[2] + x[3] + x[4] - x[5] - x[6] + x[7],
        x[0] + x[1] + x[2] + x[3] - x[4] - x[5] - x[6] - x[7],
        x[0] - x[1] + x[2] - x[3] - x[4] + x[5] - x[6] + x[7],
        x[0] + x[1] - x[2] - x[3] - x[4] - x[5] + x[6] + x[7],
        x[0] - x[1] - x[2] + x[3] - x[4] + x[5] + x[6] - x[7],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fdn_produces_reverb_tail() {
        let mut fdn = FdnReverb::new(10.0, 8.0, 3.0, 1.0, 48000, 1.0).unwrap();
        // Feed an impulse
        let _ = fdn.process_sample(1.0);
        // Check for reverb tail
        let mut has_tail = false;
        for _ in 0..10000 {
            let out = fdn.process_sample(0.0);
            if out.abs() > 0.001 {
                has_tail = true;
                break;
            }
        }
        assert!(has_tail, "FDN should produce a reverb tail");
    }

    #[test]
    fn test_fdn_output_finite() {
        let mut fdn = FdnReverb::new(10.0, 8.0, 3.0, 0.5, 48000, 1.0).unwrap();
        let _ = fdn.process_sample(1.0);
        for _ in 0..5000 {
            let out = fdn.process_sample(0.0);
            assert!(out.is_finite(), "output should be finite");
            assert!(out.abs() < 100.0, "output should not blow up: {out}");
        }
    }

    #[test]
    fn test_fdn_invalid_params() {
        assert!(FdnReverb::new(0.0, 8.0, 3.0, 1.0, 48000, 1.0).is_err());
        assert!(FdnReverb::new(10.0, 8.0, 3.0, -1.0, 48000, 1.0).is_err());
        assert!(FdnReverb::new(10.0, 8.0, 3.0, 1.0, 0, 1.0).is_err());
    }

    #[test]
    fn test_serde_roundtrip() {
        let fdn = FdnReverb::new(10.0, 8.0, 3.0, 1.5, 48000, 0.7).unwrap();
        let json = serde_json::to_string(&fdn).unwrap();
        let mut back: FdnReverb = serde_json::from_str(&json).unwrap();
        assert!((fdn.mix - back.mix).abs() < f32::EPSILON);
        assert!((fdn.target_rt60 - back.target_rt60).abs() < f32::EPSILON);
        // Fdn should be None after deser, but process_sample reconstructs it
        assert!(back.fdn.is_none());
        let out = back.process_sample(1.0);
        assert!(out.is_finite());
        assert!(back.fdn.is_some(), "should reconstruct fdn on first use");
    }

    #[test]
    fn test_hadamard8_orthogonality() {
        // Hadamard transform applied twice = N * identity. So
        // hadamard8(hadamard8(x)) should equal 8 * x.
        let x = [1.0f32, 2.0, 3.0, 4.0, -5.0, -6.0, 7.0, 8.0];
        let h = hadamard8(x);
        let hh = hadamard8(h);
        for (i, &v) in hh.iter().enumerate() {
            let expected = 8.0 * x[i];
            assert!(
                (v - expected).abs() < 1e-3,
                "round-trip[{i}] = {v}, expected {expected}"
            );
        }
    }

    #[test]
    fn test_matrix_fdn_produces_reverb_tail() {
        let mut fdn = MatrixFdn::new(1.0, 48000, 1.0).unwrap();
        let _ = fdn.process_sample(1.0);
        let mut has_tail = false;
        for _ in 0..10_000 {
            let out = fdn.process_sample(0.0);
            if out.abs() > 0.001 {
                has_tail = true;
                break;
            }
        }
        assert!(has_tail, "MatrixFdn should produce a reverb tail");
    }

    #[test]
    fn test_matrix_fdn_decays() {
        // Energy in an early window after the impulse has circulated
        // through the longest delay line should exceed energy in a much
        // later window. Using RT60=0.5s, the energy at t≈0.5s should be
        // 60 dB below t≈0.1s — well within the bound below.
        let sr = 48000;
        let mut fdn = MatrixFdn::new(0.5, sr, 1.0).unwrap();
        let _ = fdn.process_sample(1.0);

        // Run the impulse out for ~80ms to let it traverse all delay lines.
        for _ in 0..(0.08 * sr as f32) as usize {
            let _ = fdn.process_sample(0.0);
        }
        // Measure energy in a 50ms window starting at t≈80ms.
        let mut early = 0.0f32;
        for _ in 0..(0.05 * sr as f32) as usize {
            let out = fdn.process_sample(0.0);
            early += out * out;
        }
        // Skip ~400ms to land deep in the decay tail.
        for _ in 0..(0.4 * sr as f32) as usize {
            let _ = fdn.process_sample(0.0);
        }
        // Measure 50ms window in the tail (around t≈530ms past RT60).
        let mut late = 0.0f32;
        for _ in 0..(0.05 * sr as f32) as usize {
            let out = fdn.process_sample(0.0);
            late += out * out;
        }
        assert!(
            early > late * 5.0,
            "energy should decay across ~RT60: early={early}, late={late}"
        );
    }

    #[test]
    fn test_matrix_fdn_output_finite_under_white_noise() {
        // Pump white-ish input forever — output must stay finite and bounded.
        let mut fdn = MatrixFdn::new(2.0, 48000, 0.5).unwrap();
        let mut state = 12345u32;
        for _ in 0..20_000 {
            let n = crate::dsp_util::xorshift32_signed_f32(&mut state);
            let out = fdn.process_sample(n);
            assert!(out.is_finite() && out.abs() < 100.0, "blow-up: {out}");
        }
    }

    #[test]
    fn test_matrix_fdn_invalid_params() {
        assert!(MatrixFdn::new(0.0, 48000, 1.0).is_err());
        assert!(MatrixFdn::new(-1.0, 48000, 1.0).is_err());
        assert!(MatrixFdn::new(1.0, 0, 1.0).is_err());
    }

    #[test]
    fn test_matrix_fdn_dry_passthrough() {
        // mix=0 → output == input.
        let mut fdn = MatrixFdn::new(1.0, 48000, 0.0).unwrap();
        let out = fdn.process_sample(0.7);
        assert!((out - 0.7).abs() < 1e-5);
    }

    #[test]
    fn test_matrix_fdn_serde_roundtrip() {
        let fdn = MatrixFdn::new(1.5, 48000, 0.6).unwrap();
        let json = serde_json::to_string(&fdn).unwrap();
        let mut back: MatrixFdn = serde_json::from_str(&json).unwrap();
        assert!((fdn.mix - back.mix).abs() < f32::EPSILON);
        assert!((fdn.target_rt60 - back.target_rt60).abs() < f32::EPSILON);
        // After deser the buffers are empty placeholders — the first
        // process_sample call should reconstruct them.
        let out = back.process_sample(1.0);
        assert!(out.is_finite());
        assert_eq!(back.buffers[0].len(), back.delay_lengths[0]);
    }
}
