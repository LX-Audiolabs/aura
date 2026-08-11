//! Karplus-Strong plucked string oscillator.
//!
//! Ported from fundsp `src/oscillator.rs` (MIT / Apache-2.0).
//! Original authors: Sami Perttu and contributors.
//!
//! Delay line + first-order allpass tuning + 3-tap FIR damping in the
//! feedback path. Delay length sets pitch; gain-per-second sets decay.

/// Karplus-Strong plucked string oscillator.
///
/// # Examples
///
/// ```rust
/// use aura_dsp::oscillator::Pluck;
/// let mut pluck = Pluck::new(44100.0, 440.0, 0.99, 0.5).unwrap();
/// pluck.input_excitation(1.0);
/// let sample = pluck.next_sample(0.0);
/// assert!(sample.is_finite());
/// ```
#[derive(Debug, Clone)]
pub struct Pluck {
    /// Delay line (ring buffer)
    line: Vec<f32>,
    /// Read position in the delay line
    pos: usize,
    /// Amplitude multiplier per sample (from `gain_per_second`)
    gain: f32,
    /// Stored so [`set_frequency`] can recompute `gain` correctly
    gain_per_second: f32,
    /// Frequency in Hz
    frequency: f32,
    /// Sample rate
    sample_rate: f32,

    /// Allpass tuning filter state
    ap_x1: f32,
    ap_y1: f32,
    ap_coeff: f32,
    /// FIR damping filter state (3-tap)
    damp_z1: f32,
    damp_z2: f32,
    damp_b0: f32,
    damp_b1: f32,
    damp_b2: f32,
}

impl Pluck {
    /// Create a new Karplus-Strong oscillator.
    ///
    /// - `frequency` — pitch in Hz.
    /// - `gain_per_second` — amplitude multiplier per second (≤ 1.0).
    ///   Values near 1.0 produce long decays; 0.5 decays quickly.
    /// - `high_frequency_damping` — 0…1, higher = darker tone.
    ///
    /// # Errors
    ///
    /// Returns error if `sample_rate` or `frequency` is invalid.
    pub fn new(
        sample_rate: f32,
        frequency: f32,
        gain_per_second: f32,
        high_frequency_damping: f32,
    ) -> crate::error::Result<Self> {
        if let Some(e) = crate::error::validate_sample_rate(sample_rate) {
            return Err(e);
        }
        if let Some(e) = crate::error::validate_frequency(frequency, sample_rate) {
            return Err(e);
        }

        let damping = high_frequency_damping.clamp(0.0, 1.0);
        // 3-tap FIR: [d, 1-2d, d] with d = (1 - damping) / 2  (fundsp fir3)
        let d = (1.0 - damping) * 0.5;
        let gps = gain_per_second.clamp(0.0, 1.0);
        let gain = gps.powf(1.0 / frequency);

        let mut pluck = Self {
            line: Vec::new(),
            pos: 0,
            gain,
            gain_per_second: gps,
            frequency,
            sample_rate,
            ap_x1: 0.0,
            ap_y1: 0.0,
            ap_coeff: 0.0,
            damp_z1: 0.0,
            damp_z2: 0.0,
            damp_b0: d,
            damp_b1: 1.0 - 2.0 * d,
            damp_b2: d,
        };
        pluck.initialize_line();
        Ok(pluck)
    }

    /// Compute delay line length and initialize with noise.
    ///
    /// Matches fundsp: total delay = `sr/freq − 1` (one sample for the
    /// damping FIR), remainder taken by the allpass fractional delay.
    fn initialize_line(&mut self) {
        let epsilon = 0.2;
        // Damping FIR contributes ~1 sample of group delay (fundsp).
        let total_delay = (self.sample_rate / self.frequency) - 1.0;
        let loop_delay = (total_delay - epsilon).floor().max(2.0) as usize;
        let allpass_delay = total_delay - loop_delay as f32;

        // Allpass: delay ≈ (1 − coeff)/(1 + coeff) for fractional part in
        // [epsilon, epsilon+1] (fundsp Allpole set_delay).
        let frac = (allpass_delay - epsilon).clamp(0.0, 1.0);
        self.ap_coeff = (1.0 - frac) / (1.0 + frac);
        self.ap_x1 = 0.0;
        self.ap_y1 = 0.0;
        self.damp_z1 = 0.0;
        self.damp_z2 = 0.0;

        self.line.resize(loop_delay, 0.0);
        let mut mean: f32 = 0.0;
        let mut seed: u32 = 0xDEAD_BEEF;
        for item in &mut self.line {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let noise = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
            *item = noise;
            mean += *item;
        }
        mean /= self.line.len() as f32;
        for item in &mut self.line {
            *item -= mean;
        }

        self.pos = 0;
    }

    /// Add external excitation at the current write position (pluck burst).
    #[inline]
    pub fn input_excitation(&mut self, excitation: f32) {
        if !self.line.is_empty() {
            self.line[self.pos] += excitation;
        }
    }

    /// Generate next sample.
    ///
    /// `excitation` — external input added to the string (0.0 for free decay).
    #[inline]
    pub fn next_sample(&mut self, excitation: f32) -> f32 {
        if self.line.is_empty() {
            return 0.0;
        }

        let output = self.line[self.pos] * self.gain + excitation;

        // FIR damping filter
        let filtered =
            self.damp_b0 * output + self.damp_b1 * self.damp_z1 + self.damp_b2 * self.damp_z2;
        self.damp_z2 = self.damp_z1;
        self.damp_z1 = output;

        // Allpass tuning filter
        let ap_out = self.ap_coeff * (filtered - self.ap_y1) + self.ap_x1;
        self.ap_x1 = filtered;
        self.ap_y1 = ap_out;

        self.line[self.pos] = ap_out;
        self.pos += 1;
        if self.pos >= self.line.len() {
            self.pos = 0;
        }

        ap_out
    }

    /// Reset the string with new random excitation noise.
    #[inline]
    pub fn reset(&mut self) {
        self.initialize_line();
    }

    /// Set frequency in Hz. Reinitializes the delay line and recomputes gain.
    #[inline]
    pub fn set_frequency(&mut self, frequency: f32) {
        if let Some(e) = crate::error::validate_frequency(frequency, self.sample_rate) {
            let _ = e;
            return;
        }
        if (self.frequency - frequency).abs() <= 0.01 {
            return;
        }
        self.frequency = frequency;
        // gain = gain_per_second^(1/freq) — must recompute from stored GPS,
        // not powf(old_gain, old/new) after overwriting frequency.
        self.gain = self.gain_per_second.powf(1.0 / frequency);
        self.initialize_line();
    }

    /// Current pitch in Hz.
    #[inline]
    #[must_use]
    pub fn frequency(&self) -> f32 {
        self.frequency
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pluck_produces_decaying_sound() {
        let mut pluck = Pluck::new(44100.0, 440.0, 0.99, 0.5).unwrap();
        pluck.input_excitation(1.0);

        let mut peak = 0.0f32;
        let mut last_abs = 0.0f32;
        for _ in 0..500 {
            let s = pluck.next_sample(0.0);
            assert!(s.is_finite());
            peak = peak.max(s.abs());
            last_abs = s.abs();
        }
        assert!(peak > 0.1, "should produce audible sound");
        assert!(last_abs > 0.001, "should decay slowly with gain=0.99");
    }

    #[test]
    fn pluck_fast_decay() {
        let mut pluck = Pluck::new(44100.0, 440.0, 0.1, 0.5).unwrap();
        pluck.input_excitation(1.0);

        // gain_per_second = 0.1 → ~20 dB/s; after ~1 s should be tiny.
        for _ in 0..48_000 {
            pluck.next_sample(0.0);
        }
        let s = pluck.next_sample(0.0);
        assert!(
            s.abs() < 0.05,
            "should be near-silent after fast decay, got {s}"
        );
    }

    #[test]
    fn pluck_reset_produces_sound() {
        let mut pluck = Pluck::new(44100.0, 440.0, 0.99, 0.5).unwrap();
        for _ in 0..5000 {
            pluck.next_sample(0.0);
        }
        pluck.reset();
        pluck.input_excitation(1.0);
        let s = pluck.next_sample(0.0);
        assert!(s.abs() > 0.01, "reset should re-excite the string");
    }

    #[test]
    fn pluck_set_frequency_updates_gain() {
        let mut pluck = Pluck::new(44100.0, 220.0, 0.5, 0.3).unwrap();
        let gain_low = pluck.gain;
        pluck.set_frequency(880.0);
        // gain = 0.5^(1/f): higher f → gain closer to 1
        assert!(
            pluck.gain > gain_low,
            "higher pitch should have larger per-sample gain for same GPS"
        );
        assert!((pluck.gain - 0.5f32.powf(1.0 / 880.0)).abs() < 1e-5);
    }

    #[test]
    fn pluck_invalid_params() {
        assert!(Pluck::new(0.0, 440.0, 0.99, 0.5).is_err());
        assert!(Pluck::new(44100.0, 0.0, 0.99, 0.5).is_err());
    }
}
