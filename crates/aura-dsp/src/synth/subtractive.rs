//! Subtractive synthesis: oscillator(s) → filter → amplitude envelope.
//!
//! Provides a composable signal chain for classic analog-style synthesis.
//! The consumer (dhvani) handles voice allocation and polyphony — this
//! module provides a single-voice subtractive signal path.

use serde::{Deserialize, Serialize};

use crate::envelope::Adsr;
use crate::error::Result;
use crate::filter::StateVariableFilter;
use crate::oscillator::{Oscillator, Waveform};

/// Single-voice subtractive synthesis signal chain.
///
/// Signal flow: `oscillator → filter → amplitude envelope → output`
///
/// Optionally supports a second oscillator mixed with the first,
/// filter envelope modulation, and LFO modulation (via external input).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtractiveSynth {
    /// Primary oscillator.
    osc1: Oscillator,
    /// Optional second oscillator (mixed with osc1).
    osc2: Option<Oscillator>,
    /// Oscillator 2 mix level (0.0 to 1.0).
    osc2_mix: f32,
    /// State variable filter.
    filter: StateVariableFilter,
    /// Amplitude envelope.
    amp_env: Adsr,
    /// Filter envelope (modulates filter cutoff).
    filter_env: Adsr,
    /// Filter envelope depth in octaves (-4.0 to +4.0).
    filter_env_depth: f32,
    /// Base filter cutoff frequency in Hz.
    base_cutoff: f32,
    /// Sample rate in Hz.
    sample_rate: f32,
    /// Previous modulated cutoff (for skipping redundant coefficient recalculation).
    #[serde(skip)]
    prev_modulated_cutoff: f32,
}

impl SubtractiveSynth {
    /// Create a new subtractive synth voice.
    ///
    /// # Errors
    ///
    /// Returns error if any parameter is invalid.
    pub fn new(
        waveform: Waveform,
        frequency: f32,
        cutoff: f32,
        resonance: f32,
        sample_rate: f32,
    ) -> Result<Self> {
        let osc1 = Oscillator::new(waveform, frequency, sample_rate)?;
        let filter = StateVariableFilter::new(cutoff, resonance.max(0.1), sample_rate)?;
        let amp_env = Adsr::with_sample_rate(0.01, 0.1, 0.7, 0.3, sample_rate)?;
        let filter_env = Adsr::with_sample_rate(0.01, 0.2, 0.5, 0.5, sample_rate)?;

        Ok(Self {
            osc1,
            osc2: None,
            osc2_mix: 0.5,
            filter,
            amp_env,
            filter_env,
            filter_env_depth: 2.0,
            base_cutoff: cutoff,
            sample_rate,
            prev_modulated_cutoff: cutoff,
        })
    }

    /// Enable a second oscillator.
    ///
    /// # Errors
    ///
    /// Returns error if parameters are invalid.
    pub fn set_osc2(&mut self, waveform: Waveform, frequency: f32, mix: f32) -> Result<()> {
        self.osc2 = Some(Oscillator::new(waveform, frequency, self.sample_rate)?);
        self.osc2_mix = mix.clamp(0.0, 1.0);
        Ok(())
    }

    /// Disable the second oscillator.
    pub fn clear_osc2(&mut self) {
        self.osc2 = None;
    }

    /// Trigger the voice (note on).
    pub fn note_on(&mut self) {
        self.amp_env.gate_on();
        self.filter_env.gate_on();
    }

    /// Release the voice (note off).
    pub fn note_off(&mut self) {
        self.amp_env.gate_off();
        self.filter_env.gate_off();
    }

    /// Set the oscillator frequency (e.g., from MIDI note).
    ///
    /// # Errors
    ///
    /// Returns error if frequency is invalid.
    pub fn set_frequency(&mut self, freq: f32) -> Result<()> {
        self.osc1.set_frequency(freq)?;
        if let Some(ref mut osc2) = self.osc2 {
            let _ = osc2.set_frequency(freq);
        }
        Ok(())
    }

    /// Set filter envelope depth in octaves.
    pub fn set_filter_env_depth(&mut self, octaves: f32) {
        self.filter_env_depth = octaves.clamp(-4.0, 4.0);
    }

    /// Set the base filter cutoff.
    ///
    /// # Errors
    ///
    /// Returns error if cutoff is invalid.
    pub fn set_cutoff(&mut self, cutoff: f32) -> Result<()> {
        self.base_cutoff = cutoff;
        self.filter.set_params(cutoff, self.filter.q())
    }

    /// Generate the next sample.
    #[inline]
    #[must_use]
    pub fn next_sample(&mut self) -> f32 {
        // Oscillator mix
        let mut osc_out = self.osc1.next_sample();
        if let Some(ref mut osc2) = self.osc2 {
            let o2 = osc2.next_sample();
            osc_out = osc_out * (1.0 - self.osc2_mix) + o2 * self.osc2_mix;
        }

        // Filter with envelope modulation
        let filter_mod = self.filter_env.next_value() * self.filter_env_depth;
        let modulated_cutoff = self.base_cutoff * filter_mod.exp2();
        let clamped = modulated_cutoff.clamp(20.0, self.sample_rate * 0.49);
        // Only recompute filter coefficients when cutoff changes meaningfully
        if (clamped - self.prev_modulated_cutoff).abs() > 0.5 {
            let _ = self.filter.set_params(clamped, self.filter.q());
            self.prev_modulated_cutoff = clamped;
        }
        let filtered = self.filter.process_sample(osc_out).low_pass;

        // Amplitude envelope
        let amp = self.amp_env.next_value();

        filtered * amp
    }

    /// Fill a buffer with samples.
    #[inline]
    pub fn fill_buffer(&mut self, buffer: &mut [f32]) {
        for s in buffer.iter_mut() {
            *s = self.next_sample();
        }
    }

    /// Check if the voice is still active (envelope not idle).
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.amp_env.is_active()
    }

    /// Returns a reference to the amplitude envelope.
    #[must_use]
    pub fn amp_env(&self) -> &Adsr {
        &self.amp_env
    }

    /// Returns a mutable reference to the amplitude envelope.
    pub fn amp_env_mut(&mut self) -> &mut Adsr {
        &mut self.amp_env
    }

    /// Returns a mutable reference to the filter envelope.
    pub fn filter_env_mut(&mut self) -> &mut Adsr {
        &mut self.filter_env
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_output() {
        let mut synth =
            SubtractiveSynth::new(Waveform::Saw, 440.0, 2000.0, 0.707, 44100.0).unwrap();
        synth.note_on();
        let mut buf = [0.0f32; 1024];
        synth.fill_buffer(&mut buf);
        assert!(buf.iter().any(|&s| s.abs() > 0.01), "should produce output");
        assert!(buf.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn test_note_off_decays() {
        let mut synth =
            SubtractiveSynth::new(Waveform::Sine, 440.0, 5000.0, 0.707, 44100.0).unwrap();
        synth.note_on();
        for _ in 0..5000 {
            let _ = synth.next_sample();
        }
        synth.note_off();
        for _ in 0..50000 {
            let _ = synth.next_sample();
        }
        assert!(!synth.is_active(), "should be idle after release");
    }

    #[test]
    fn test_two_oscillators() {
        let mut synth =
            SubtractiveSynth::new(Waveform::Saw, 440.0, 3000.0, 0.707, 44100.0).unwrap();
        synth.set_osc2(Waveform::Square, 441.0, 0.5).unwrap();
        synth.note_on();
        let mut buf = [0.0f32; 512];
        synth.fill_buffer(&mut buf);
        assert!(buf.iter().any(|&s| s.abs() > 0.01));
    }

    #[test]
    fn test_serde_roundtrip() {
        let synth = SubtractiveSynth::new(Waveform::Saw, 440.0, 2000.0, 0.707, 44100.0).unwrap();
        let json = serde_json::to_string(&synth).unwrap();
        let back: SubtractiveSynth = serde_json::from_str(&json).unwrap();
        assert!((synth.base_cutoff - back.base_cutoff).abs() < f32::EPSILON);
    }
}
