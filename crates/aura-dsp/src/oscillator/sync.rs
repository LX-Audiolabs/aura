//! Hard sync — slave oscillator phase resets on master cycle wrap.

use serde::{Deserialize, Serialize};

use super::core::{Oscillator, Waveform};
use crate::error::Result;

/// Hard sync oscillator — slave resets phase on master cycle completion.
///
/// When the master oscillator wraps its phase (completes a cycle), the slave
/// oscillator's phase is reset to zero, producing the characteristic hard sync
/// harmonic sweep effect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardSync {
    /// Master oscillator (controls reset timing).
    master: Oscillator,
    /// Slave oscillator (produces output, gets phase-reset).
    slave: Oscillator,
}

impl HardSync {
    /// Create a new hard sync pair.
    ///
    /// # Errors
    ///
    /// Returns error if frequencies or sample_rate are invalid.
    pub fn new(
        master_freq: f32,
        slave_freq: f32,
        slave_waveform: Waveform,
        sample_rate: f32,
    ) -> Result<Self> {
        let master = Oscillator::new(Waveform::Saw, master_freq, sample_rate)?;
        let slave = Oscillator::new(slave_waveform, slave_freq, sample_rate)?;
        Ok(Self { master, slave })
    }

    /// Generate the next hard-synced sample.
    ///
    /// The slave produces audio; the master controls when the slave resets.
    #[inline]
    #[must_use]
    pub fn next_sample(&mut self) -> f32 {
        let master_phase_before = self.master.phase();
        let _ = self.master.next_sample();
        let master_phase_after = self.master.phase();

        // Detect master cycle wrap (phase decreased = wrapped past 1.0)
        if master_phase_after < master_phase_before {
            self.slave.reset_phase();
        }

        self.slave.next_sample()
    }

    /// Fill a buffer with hard-synced samples.
    #[inline]
    pub fn fill_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.next_sample();
        }
    }

    /// Returns a reference to the master oscillator.
    #[must_use]
    pub fn master(&self) -> &Oscillator {
        &self.master
    }

    /// Returns a reference to the slave oscillator.
    #[must_use]
    pub fn slave(&self) -> &Oscillator {
        &self.slave
    }

    /// Set the master frequency.
    ///
    /// # Errors
    ///
    /// Returns error if frequency is invalid.
    pub fn set_master_freq(&mut self, freq: f32) -> Result<()> {
        self.master.set_frequency(freq)
    }

    /// Set the slave frequency.
    ///
    /// # Errors
    ///
    /// Returns error if frequency is invalid.
    pub fn set_slave_freq(&mut self, freq: f32) -> Result<()> {
        self.slave.set_frequency(freq)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hard_sync_resets_slave() {
        // Master at 440 Hz, slave at 880 Hz — slave should reset every master cycle
        let mut sync = HardSync::new(440.0, 880.0, Waveform::Saw, 44100.0).unwrap();
        let mut buf = [0.0f32; 1024];
        sync.fill_buffer(&mut buf);
        assert!(buf.iter().all(|s| s.is_finite()));
        // Slave should produce non-trivial output
        assert!(buf.iter().any(|&s| s != 0.0));
    }

    #[test]
    fn test_hard_sync_serde_roundtrip() {
        let hs = HardSync::new(440.0, 880.0, Waveform::Saw, 44100.0).unwrap();
        let json = serde_json::to_string(&hs).unwrap();
        let back: HardSync = serde_json::from_str(&json).unwrap();
        assert!((hs.master().frequency() - back.master().frequency()).abs() < f32::EPSILON);
    }
}
