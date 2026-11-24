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
    /// Post-event half of the two-point BLEP correction.
    #[serde(skip)]
    pending_blep: f32,
}

impl HardSync {
    /// Create a new hard sync pair.
    ///
    /// # Errors
    ///
    /// Returns error if frequencies or `sample_rate` are invalid.
    pub fn new(
        master_freq: f32,
        slave_freq: f32,
        slave_waveform: Waveform,
        sample_rate: f32,
    ) -> Result<Self> {
        let master = Oscillator::new(Waveform::Saw, master_freq, sample_rate)?;
        let slave = Oscillator::new(slave_waveform, slave_freq, sample_rate)?;
        Ok(Self {
            master,
            slave,
            pending_blep: 0.0,
        })
    }

    /// Generate the next hard-synced sample.
    ///
    /// The slave produces audio; the master controls when the slave resets.
    #[inline]
    #[must_use]
    pub fn next_sample(&mut self) -> f32 {
        let master_phase_before = self.master.phase();
        let master_dt = self.master.phase_increment();
        let slave_phase = self.slave.phase();
        let slave_dt = self.slave.phase_increment();
        let (edges, count) = self.slave.edges();
        let sample = if count == 0 {
            self.slave.next_sample()
        } else {
            let raw = self.slave.raw_sample_at(slave_phase);
            let _ = self.slave.next_sample();
            raw
        };
        let mut output = sample + self.pending_blep;
        self.pending_blep = 0.0;
        let _ = self.master.next_sample();

        if master_phase_before + master_dt >= 1.0 {
            let fraction = ((1.0 - master_phase_before) / master_dt).clamp(0.0, 1.0);
            self.correct_edges(
                slave_phase,
                slave_dt * fraction,
                0.0,
                slave_dt,
                &edges[..count],
                &mut output,
            );
            let event_phase = (slave_phase + fraction * slave_dt).rem_euclid(1.0);
            let jump = self.slave.raw_sample_at(0.0) - self.slave.raw_sample_at(event_phase);
            // The wrap lies between this sample and the next: correct both sides.
            output += 0.5 * jump * (1.0 - fraction).powi(2);
            self.pending_blep += -0.5 * jump * fraction * fraction;
            self.slave.set_phase((1.0 - fraction) * slave_dt);
            self.correct_edges(
                0.0,
                slave_dt * (1.0 - fraction),
                fraction,
                slave_dt,
                &edges[..count],
                &mut output,
            );
        } else {
            self.correct_edges(
                slave_phase,
                slave_dt,
                0.0,
                slave_dt,
                &edges[..count],
                &mut output,
            );
        }
        output
    }

    /// Correct natural discontinuities only on the part of the trajectory actually traversed.
    fn correct_edges(
        &mut self,
        start: f32,
        advance: f32,
        offset: f32,
        dt: f32,
        edges: &[f32],
        output: &mut f32,
    ) {
        for &edge in edges {
            let distance = (edge - start).rem_euclid(1.0);
            if distance > 0.0 && distance <= advance {
                let fraction = offset + distance / dt;
                let jump = self.slave.raw_sample_at((edge + 1e-6).rem_euclid(1.0))
                    - self.slave.raw_sample_at((edge - 1e-6).rem_euclid(1.0));
                *output += 0.5 * jump * (1.0 - fraction).powi(2);
                self.pending_blep -= 0.5 * jump * fraction * fraction;
            }
        }
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
    fn regression_sync_corrects_reset_jump() {
        let mut sync = HardSync::new(10_000.0, 7_000.0, Waveform::Sine, 48_000.0).unwrap();
        for _ in 0..4 {
            let _ = sync.next_sample();
        }
        let dry = (std::f32::consts::TAU * 4.0 * 7_000.0 / 48_000.0).sin();
        let corrected = sync.next_sample();
        let jump = -(std::f32::consts::TAU * 0.7).sin();
        assert!((corrected - (dry + 0.5 * jump * 0.2 * 0.2)).abs() < 1e-5);
    }

    #[test]
    fn regression_sync_retains_subsample_phase() {
        let mut sync = HardSync::new(10_000.0, 7_000.0, Waveform::Sine, 48_000.0).unwrap();
        for _ in 0..5 {
            let _ = sync.next_sample();
        }
        let expected = (5.0 - 4.8) * 7_000.0 / 48_000.0;
        assert!((sync.slave().phase() - expected).abs() < 1e-6);
    }

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
