//! Modulation sources: LFO, FM synthesis, and ring modulation.

use serde::{Deserialize, Serialize};

use crate::error::{self, Result};
use crate::oscillator::{Oscillator, Waveform};

/// LFO waveform shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum LfoShape {
    /// Sine wave.
    Sine,
    /// Triangle wave.
    Triangle,
    /// Square wave (bipolar).
    Square,
    /// Ascending sawtooth (ramp up).
    SawUp,
    /// Descending sawtooth (ramp down).
    SawDown,
    /// Sample-and-hold (random step at each cycle).
    SampleAndHold,
}

/// LFO output mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum LfoMode {
    /// Bipolar output: -1.0 to +1.0.
    Bipolar,
    /// Unipolar output: 0.0 to +1.0.
    Unipolar,
}

fn default_sh_value() -> f32 {
    // Non-zero initial S&H value so first cycle isn't silent
    0.5
}

fn default_rng_state() -> u32 {
    // Must be non-zero or xorshift produces 0 forever
    42
}

/// Low-frequency oscillator for modulation.
///
/// Supports 6 waveform shapes with bipolar or unipolar output modes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lfo {
    /// LFO waveform shape.
    shape: LfoShape,
    /// Frequency in Hz.
    frequency: f32,
    /// Sample rate in Hz.
    sample_rate: f32,
    /// Phase accumulator (0.0 to 1.0).
    phase: f32,
    /// Output mode (bipolar or unipolar).
    mode: LfoMode,
    /// Modulation depth (amplitude scaling, 0.0 to 1.0).
    pub depth: f32,
    /// Current sample-and-hold value.
    #[serde(skip, default = "default_sh_value")]
    sh_value: f32,
    /// PRNG state for sample-and-hold.
    #[serde(skip, default = "default_rng_state")]
    rng_state: u32,
}

impl Lfo {
    /// Create a new LFO with a given shape.
    ///
    /// # Errors
    ///
    /// Returns error if frequency or sample_rate is invalid.
    pub fn new(shape: LfoShape, frequency: f32, sample_rate: f32) -> Result<Self> {
        if let Some(e) = error::validate_sample_rate(sample_rate) {
            return Err(e);
        }
        if frequency < 0.0 || !frequency.is_finite() {
            return Err(crate::NaadError::InvalidParameter {
                name: "frequency".to_string(),
                reason: "must be >= 0 and finite".to_string(),
            });
        }

        // Initialize S&H with first random value so first cycle isn't silent
        let mut rng = 42u32;
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        let initial_sh = (rng as f32 / u32::MAX as f32) * 2.0 - 1.0;

        Ok(Self {
            shape,
            frequency,
            sample_rate,
            phase: 0.0,
            mode: LfoMode::Bipolar,
            depth: 1.0,
            sh_value: initial_sh,
            rng_state: rng,
        })
    }

    /// Create an LFO from a legacy `Waveform` enum (for backward compatibility).
    ///
    /// Maps: Sine→Sine, Triangle→Triangle, Square→Square, Saw→SawDown.
    ///
    /// # Errors
    ///
    /// Returns error if frequency or sample_rate is invalid.
    pub fn from_waveform(waveform: Waveform, frequency: f32, sample_rate: f32) -> Result<Self> {
        let shape = match waveform {
            Waveform::Sine => LfoShape::Sine,
            Waveform::Triangle => LfoShape::Triangle,
            Waveform::Square => LfoShape::Square,
            Waveform::Saw => LfoShape::SawDown,
            _ => LfoShape::Sine,
        };
        Self::new(shape, frequency, sample_rate)
    }

    /// Generate the next modulation value (scaled by depth).
    #[inline]
    #[must_use]
    pub fn next_value(&mut self) -> f32 {
        let raw = self.raw_sample();

        // Advance phase
        let dt = self.frequency / self.sample_rate;
        let prev_phase = self.phase;
        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        // Update S&H on cycle wrap
        if matches!(self.shape, LfoShape::SampleAndHold) && self.phase < prev_phase {
            self.rng_state ^= self.rng_state << 13;
            self.rng_state ^= self.rng_state >> 17;
            self.rng_state ^= self.rng_state << 5;
            self.sh_value = (self.rng_state as f32 / u32::MAX as f32) * 2.0 - 1.0;
        }

        let output = match self.mode {
            LfoMode::Bipolar => raw,
            LfoMode::Unipolar => (raw + 1.0) * 0.5,
        };

        output * self.depth
    }

    /// Compute the raw bipolar sample for the current phase.
    #[inline]
    fn raw_sample(&self) -> f32 {
        let t = self.phase;
        match self.shape {
            LfoShape::Sine => (t * std::f32::consts::TAU).sin(),
            LfoShape::Triangle => {
                if t < 0.25 {
                    4.0 * t
                } else if t < 0.75 {
                    2.0 - 4.0 * t
                } else {
                    4.0 * t - 4.0
                }
            }
            LfoShape::Square => {
                if t < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            LfoShape::SawUp => 2.0 * t - 1.0,
            LfoShape::SawDown => 1.0 - 2.0 * t,
            LfoShape::SampleAndHold => self.sh_value,
        }
    }

    /// Set the LFO frequency.
    ///
    /// # Errors
    ///
    /// Returns error if frequency is invalid.
    pub fn set_frequency(&mut self, freq: f32) -> Result<()> {
        if freq < 0.0 || !freq.is_finite() {
            return Err(crate::NaadError::InvalidParameter {
                name: "frequency".to_string(),
                reason: "must be >= 0 and finite".to_string(),
            });
        }
        self.frequency = freq;
        Ok(())
    }

    /// Set the LFO shape.
    pub fn set_shape(&mut self, shape: LfoShape) {
        self.shape = shape;
    }

    /// Set the output mode (bipolar or unipolar).
    pub fn set_mode(&mut self, mode: LfoMode) {
        self.mode = mode;
    }

    /// Returns the current shape.
    #[inline]
    #[must_use]
    pub fn shape(&self) -> LfoShape {
        self.shape
    }

    /// Returns the current mode.
    #[inline]
    #[must_use]
    pub fn mode(&self) -> LfoMode {
        self.mode
    }
}

/// Trait for modulation sources.
pub trait ModulationSource {
    /// Generate the next modulation value.
    fn next_modulation_value(&mut self) -> f32;
}

impl ModulationSource for Lfo {
    fn next_modulation_value(&mut self) -> f32 {
        self.next_value()
    }
}

/// Two-operator FM (Frequency Modulation) modulator.
///
/// The modulator oscillator's output is scaled by `mod_index` and added to
/// the carrier frequency to produce frequency modulation and sidebands.
///
/// For multi-operator algorithms (DX-style 4-op / 6-op stacks, parallel,
/// feedback), use [`crate::synth::fm::FmSynthEngine`] instead — this type
/// is just the simple 2-op modulator primitive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FmModulator {
    /// Carrier oscillator.
    carrier: Oscillator,
    /// Modulator oscillator.
    modulator: Oscillator,
    /// Modulation index (depth of FM).
    pub mod_index: f32,
    /// Base carrier frequency for FM calculation.
    carrier_base_freq: f32,
}

impl FmModulator {
    /// Create a new FM synthesizer.
    ///
    /// # Arguments
    ///
    /// * `carrier_freq` - Carrier frequency in Hz
    /// * `mod_freq` - Modulator frequency in Hz
    /// * `mod_index` - Modulation index (ratio of frequency deviation to modulator frequency)
    /// * `sample_rate` - Sample rate in Hz
    ///
    /// # Errors
    ///
    /// Returns error if frequencies or sample_rate are invalid.
    pub fn new(carrier_freq: f32, mod_freq: f32, mod_index: f32, sample_rate: f32) -> Result<Self> {
        if let Some(e) = error::validate_sample_rate(sample_rate) {
            return Err(e);
        }
        let carrier = Oscillator::new(Waveform::Sine, carrier_freq, sample_rate)?;
        let modulator = Oscillator::new(Waveform::Sine, mod_freq, sample_rate)?;
        Ok(Self {
            carrier,
            modulator,
            mod_index,
            carrier_base_freq: carrier_freq,
        })
    }

    /// Returns a shared reference to the carrier oscillator.
    #[inline]
    #[must_use]
    pub fn carrier(&self) -> &Oscillator {
        &self.carrier
    }

    /// Returns a shared reference to the modulator oscillator.
    #[inline]
    #[must_use]
    pub fn modulator(&self) -> &Oscillator {
        &self.modulator
    }

    /// Generate the next FM synthesis sample.
    ///
    /// Applies frequency modulation: carrier frequency is modulated by
    /// the modulator output scaled by `mod_index * mod_frequency`.
    /// The carrier always produces a sine wave regardless of its waveform setting.
    #[inline]
    pub fn fm_next_sample(&mut self) -> f32 {
        let mod_out = self.modulator.next_sample();

        // Calculate instantaneous carrier frequency
        let freq_deviation = mod_out * self.mod_index * self.modulator.frequency();
        let inst_freq = self.carrier_base_freq + freq_deviation;

        // Clamp to valid range
        let nyquist = self.carrier.sample_rate() / 2.0;
        let clamped_freq = inst_freq.clamp(0.1, nyquist - 1.0);

        // Phase-modulate the carrier directly (always sine)
        let dt = clamped_freq / self.carrier.sample_rate();
        self.carrier.advance_phase_sine(dt)
    }

    /// Fill a buffer with FM synthesis samples.
    #[inline]
    pub fn fill_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.fm_next_sample();
        }
    }
}

/// Ring modulator — multiplies two signals together.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RingModulator {
    /// The modulator oscillator.
    modulator: Oscillator,
}

impl RingModulator {
    /// Create a new ring modulator.
    ///
    /// # Errors
    ///
    /// Returns error if frequency or sample_rate is invalid.
    pub fn new(waveform: Waveform, mod_freq: f32, sample_rate: f32) -> Result<Self> {
        let modulator = Oscillator::new(waveform, mod_freq, sample_rate)?;
        Ok(Self { modulator })
    }

    /// Returns a shared reference to the modulator oscillator.
    #[inline]
    #[must_use]
    pub fn modulator(&self) -> &Oscillator {
        &self.modulator
    }

    /// Process a sample through ring modulation.
    ///
    /// Multiplies the input by the modulator output.
    #[inline]
    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        input * self.modulator.next_sample()
    }

    /// Process a buffer in place.
    #[inline]
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process_sample(*sample);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lfo_basic() {
        let mut lfo = Lfo::new(LfoShape::Sine, 5.0, 44100.0).unwrap();
        let val = lfo.next_value();
        assert!(val.is_finite());
    }

    #[test]
    fn test_lfo_depth() {
        let mut lfo = Lfo::new(LfoShape::Sine, 5.0, 44100.0).unwrap();
        lfo.depth = 0.5;
        for _ in 0..10000 {
            let val = lfo.next_value();
            assert!(
                val.abs() <= 0.51,
                "LFO with depth 0.5 should stay within bounds, got {val}"
            );
        }
    }

    #[test]
    fn test_lfo_all_shapes() {
        let shapes = [
            LfoShape::Sine,
            LfoShape::Triangle,
            LfoShape::Square,
            LfoShape::SawUp,
            LfoShape::SawDown,
            LfoShape::SampleAndHold,
        ];
        for shape in &shapes {
            let mut lfo = Lfo::new(*shape, 5.0, 44100.0).unwrap();
            for _ in 0..1000 {
                let val = lfo.next_value();
                assert!(
                    (-1.01..=1.01).contains(&val),
                    "LFO {shape:?} out of bipolar range: {val}"
                );
            }
        }
    }

    #[test]
    fn test_lfo_unipolar() {
        let mut lfo = Lfo::new(LfoShape::Sine, 5.0, 44100.0).unwrap();
        lfo.set_mode(LfoMode::Unipolar);
        for _ in 0..10000 {
            let val = lfo.next_value();
            assert!(
                (-0.01..=1.01).contains(&val),
                "Unipolar LFO should be 0..1, got {val}"
            );
        }
    }

    #[test]
    fn test_fm_synthesis() {
        let mut fm = FmModulator::new(440.0, 220.0, 2.0, 44100.0).unwrap();
        let mut buf = [0.0f32; 1024];
        fm.fill_buffer(&mut buf);
        assert!(buf.iter().all(|s| s.is_finite()));
        assert!(buf.iter().any(|&s| s != 0.0));
    }

    #[test]
    fn test_ring_modulator() {
        let mut ring = RingModulator::new(Waveform::Sine, 300.0, 44100.0).unwrap();
        let output = ring.process_sample(1.0);
        assert!(output.is_finite());
    }

    #[test]
    fn test_modulation_source_trait() {
        let mut lfo = Lfo::new(LfoShape::Sine, 5.0, 44100.0).unwrap();
        let val = lfo.next_modulation_value();
        assert!(val.is_finite());
    }

    #[test]
    fn test_serde_roundtrip() {
        let fm = FmModulator::new(440.0, 220.0, 2.0, 44100.0).unwrap();
        let json = serde_json::to_string(&fm).unwrap();
        let back: FmModulator = serde_json::from_str(&json).unwrap();
        assert!((fm.mod_index - back.mod_index).abs() < f32::EPSILON);
    }
}
