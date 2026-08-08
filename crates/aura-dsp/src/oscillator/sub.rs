//! Sub-oscillator — plays 1 or 2 octaves below a given frequency.

use serde::{Deserialize, Serialize};

use super::core::{Oscillator, Waveform};
use crate::error::Result;

/// Octave division for a sub-oscillator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SubOctave {
    /// One octave below (half frequency).
    Down1,
    /// Two octaves below (quarter frequency).
    Down2,
}

/// Sub-oscillator — plays 1 or 2 octaves below a given frequency.
///
/// Typically mixed with a main oscillator to add low-end body.
/// The sub-oscillator has an independent waveform selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubOscillator {
    /// The underlying oscillator.
    osc: Oscillator,
    /// Octave division.
    octave: SubOctave,
    /// Base frequency (before octave division).
    base_frequency: f32,
    /// Mix level (0.0 to 1.0).
    pub level: f32,
}

impl SubOscillator {
    /// Create a new sub-oscillator.
    ///
    /// # Errors
    ///
    /// Returns error if frequency or sample_rate is invalid.
    pub fn new(
        waveform: Waveform,
        base_frequency: f32,
        octave: SubOctave,
        sample_rate: f32,
    ) -> Result<Self> {
        let divisor = match octave {
            SubOctave::Down1 => 2.0,
            SubOctave::Down2 => 4.0,
        };
        let sub_freq = base_frequency / divisor;
        let osc = Oscillator::new(waveform, sub_freq.max(0.1), sample_rate)?;
        Ok(Self {
            osc,
            octave,
            base_frequency,
            level: 1.0,
        })
    }

    /// Generate the next sub-oscillator sample (scaled by level).
    #[inline]
    #[must_use]
    pub fn next_sample(&mut self) -> f32 {
        self.osc.next_sample() * self.level
    }

    /// Update the base frequency (sub frequency is derived automatically).
    ///
    /// # Errors
    ///
    /// Returns error if frequency is invalid.
    pub fn set_base_frequency(&mut self, freq: f32) -> Result<()> {
        let divisor = match self.octave {
            SubOctave::Down1 => 2.0,
            SubOctave::Down2 => 4.0,
        };
        // Validate before mutating state
        self.osc.set_frequency((freq / divisor).max(0.1))?;
        self.base_frequency = freq;
        Ok(())
    }

    /// Set the octave division.
    ///
    /// # Errors
    ///
    /// Returns error if the resulting frequency is invalid.
    pub fn set_octave(&mut self, octave: SubOctave) -> Result<()> {
        let divisor = match octave {
            SubOctave::Down1 => 2.0,
            SubOctave::Down2 => 4.0,
        };
        self.osc
            .set_frequency((self.base_frequency / divisor).max(0.1))?;
        self.octave = octave;
        Ok(())
    }

    /// Returns the current octave division.
    #[inline]
    #[must_use]
    pub fn octave(&self) -> SubOctave {
        self.octave
    }

    /// Returns the base frequency.
    #[inline]
    #[must_use]
    pub fn base_frequency(&self) -> f32 {
        self.base_frequency
    }

    /// Fill a buffer with sub-oscillator samples.
    #[inline]
    pub fn fill_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.next_sample();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sub_oscillator() {
        let mut sub =
            SubOscillator::new(Waveform::Square, 440.0, SubOctave::Down1, 44100.0).unwrap();
        let mut buf = [0.0f32; 1024];
        sub.fill_buffer(&mut buf);
        assert!(buf.iter().all(|s| s.is_finite()));
        assert!(buf.iter().any(|&s| s != 0.0));
    }

    #[test]
    fn test_sub_oscillator_octave() {
        let sub1 = SubOscillator::new(Waveform::Sine, 440.0, SubOctave::Down1, 44100.0).unwrap();
        let sub2 = SubOscillator::new(Waveform::Sine, 440.0, SubOctave::Down2, 44100.0).unwrap();
        // Down1 = 220 Hz, Down2 = 110 Hz
        assert!((sub1.osc.frequency() - 220.0).abs() < 0.01);
        assert!((sub2.osc.frequency() - 110.0).abs() < 0.01);
    }

    #[test]
    fn test_sub_oscillator_serde() {
        let sub = SubOscillator::new(Waveform::Square, 440.0, SubOctave::Down2, 44100.0).unwrap();
        let json = serde_json::to_string(&sub).unwrap();
        let back: SubOscillator = serde_json::from_str(&json).unwrap();
        assert_eq!(sub.octave(), back.octave());
    }
}
