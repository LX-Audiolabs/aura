//! Karplus-Strong plucked string oscillator.
//!
//! Ported from fundsp `src/oscillator.rs` (MIT / Apache-2.0).
//! Original authors: Sami Perttu and contributors.
//!
//! The Karplus-Strong algorithm simulates a plucked string using a
//! delay line with a lowpass filter in the feedback path. The delay
//! length determines pitch; damping controls decay time.

/// Karplus-Strong plucked string oscillator.
///
/// Uses a delay line + first-order allpass tuning filter + 3-tap
/// FIR damping filter. The delay line is initialized with random
/// noise (simulating the initial pluck).
///
/// # Examples
///
/// ```rust
/// use aura_dsp::oscillator::Pluck;
/// let mut pluck = Pluck::new(44100.0, 440.0, 0.99, 0.5);
/// // pluck.input_excitation(1.0); // trigger
/// let sample = pluck.next_sample(0.0);
/// ```
#[derive(Debug, Clone)]
pub struct Pluck {
    /// Delay line (ring buffer)
    line: Vec<f32>,
    /// Read position in the delay line
    pos: usize,
    /// Gain per sample (derived from gain_per_second)
    gain: f32,
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
    /// - `high_frequency_damping` — 0..1, higher = darker tone.
    pub fn new(
        sample_rate: f32,
        frequency: f32,
        gain_per_second: f32,
        high_frequency_damping: f32,
    ) -> Self {
        let damping = high_frequency_damping.clamp(0.0, 1.0);
        // 3-tap FIR: [d, 1-2d, d] with d = (1 - damping) / 2
        let d = (1.0 - damping) * 0.5;
        let damp_b0 = d;
        let damp_b1 = 1.0 - 2.0 * d;
        let damp_b2 = d;

        let gain = gain_per_second
            .clamp(0.0, 1.0)
            .powf(1.0 / frequency);

        let mut pluck = Self {
            line: Vec::new(),
            pos: 0,
            gain,
            frequency,
            sample_rate,
            ap_x1: 0.0,
            ap_y1: 0.0,
            ap_coeff: 0.0,
            damp_z1: 0.0,
            damp_z2: 0.0,
            damp_b0,
            damp_b1,
            damp_b2,
        };
        pluck.initialize_line();
        pluck
    }

    /// Compute delay line length and initialize with noise.
    fn initialize_line(&mut self) {
        // Desired delay in samples
        let target_delay = self.sample_rate / self.frequency;

        // Allpass provides fractional delay between epsilon and epsilon+1
        let epsilon = 0.2;
        let loop_delay = (target_delay - epsilon).floor() as usize;
        let allpass_delay = target_delay - loop_delay as f32;

        // Allpass coefficient for fractional delay
        // delay = (1 - coeff) / (1 + coeff)  →  coeff = (1 - delay) / (1 + delay)
        let frac = allpass_delay - epsilon;
        self.ap_coeff = (1.0 - frac) / (1.0 + frac);
        self.ap_x1 = 0.0;
        self.ap_y1 = 0.0;

        // Fill delay line with random noise
        self.line.resize(loop_delay.max(2), 0.0);
        let mut mean: f32 = 0.0;
        // Simple deterministic noise (not cryptographically random)
        let mut seed: u32 = 0xDEAD_BEEF;
        for item in &mut self.line {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let noise = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
            *item = noise * 0.5; // moderate initial amplitude
            mean += *item;
        }
        mean /= self.line.len() as f32;
        // Remove DC offset
        for item in &mut self.line {
            *item -= mean;
        }

        self.pos = 0;
    }

    /// Trigger excitation with an external input value.
    /// The input is added to the current delay line position.
    /// Use a short burst (e.g., noise or impulse) to "pluck" the string.
    #[inline]
    pub fn input_excitation(&mut self, excitation: f32) {
        // Add excitation at current position
        if !self.line.is_empty() {
            self.line[self.pos] += excitation;
        }
    }

    /// Generate next sample.
    ///
    /// `excitation` — external input added to the string (use 0.0
    /// for natural decay, or a short burst to pluck).
    #[inline]
    pub fn next_sample(&mut self, excitation: f32) -> f32 {
        if self.line.is_empty() {
            return 0.0;
        }

        // Read from delay line
        let output = self.line[self.pos] * self.gain + excitation;

        // FIR damping filter
        let filtered = self.damp_b0 * output
            + self.damp_b1 * self.damp_z1
            + self.damp_b2 * self.damp_z2;
        self.damp_z2 = self.damp_z1;
        self.damp_z1 = output;

        // Allpass tuning filter
        let ap_out = self.ap_coeff * (filtered - self.ap_y1) + self.ap_x1;
        self.ap_x1 = filtered;
        self.ap_y1 = ap_out;

        // Write back to delay line
        self.line[self.pos] = ap_out;
        self.pos += 1;
        if self.pos >= self.line.len() {
            self.pos = 0;
        }

        ap_out
    }

    /// Reset the string state with new random noise.
    #[inline]
    pub fn reset(&mut self) {
        self.initialize_line();
    }

    /// Set frequency in Hz. Reinitializes the delay line.
    #[inline]
    pub fn set_frequency(&mut self, frequency: f32) {
        if (self.frequency - frequency).abs() > 0.01 {
            self.frequency = frequency;
            self.gain = self.gain.powf(self.frequency / frequency);
            self.initialize_line();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pluck_produces_decaying_sound() {
        let mut pluck = Pluck::new(44100.0, 440.0, 0.99, 0.5);
        // Trigger with a single impulse
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
        // After 500 samples at 440 Hz, the note should still be audible
        assert!(last_abs > 0.001, "should decay slowly with gain=0.99");
    }

    #[test]
    fn pluck_fast_decay() {
        let mut pluck = Pluck::new(44100.0, 440.0, 0.1, 0.5);
        pluck.input_excitation(1.0);

        // After many samples, should be nearly silent
        for _ in 0..10000 {
            pluck.next_sample(0.0);
        }
        let s = pluck.next_sample(0.0);
        assert!(s.abs() < 0.01, "should be silent after fast decay");
    }

    #[test]
    fn pluck_reset_produces_sound() {
        let mut pluck = Pluck::new(44100.0, 440.0, 0.99, 0.5);
        // Let it decay
        for _ in 0..5000 {
            pluck.next_sample(0.0);
        }
        // Reset with new noise
        pluck.reset();
        pluck.input_excitation(1.0);
        let s = pluck.next_sample(0.0);
        assert!(s.abs() > 0.01, "reset should re-excite the string");
    }
}
