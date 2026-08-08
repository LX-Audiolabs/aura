//! Delay lines, comb filters, and allpass delay networks.

use serde::{Deserialize, Serialize};

/// Circular buffer delay line with fractional delay support.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelayLine {
    /// Internal buffer.
    buffer: Vec<f32>,
    /// Current write position.
    write_pos: usize,
    /// Maximum delay in samples.
    max_delay_samples: usize,
}

impl DelayLine {
    /// Create a new delay line with the given maximum delay in samples.
    #[must_use]
    pub fn new(max_delay_samples: usize) -> Self {
        let size = max_delay_samples.max(1);
        Self {
            buffer: vec![0.0; size],
            write_pos: 0,
            max_delay_samples: size,
        }
    }

    /// Write a sample into the delay line.
    #[inline]
    pub fn write(&mut self, sample: f32) {
        self.buffer[self.write_pos] = sample;
        self.write_pos += 1;
        if self.write_pos >= self.max_delay_samples {
            self.write_pos = 0;
        }
    }

    /// Read from the delay line with fractional delay (linear interpolation).
    ///
    /// `delay` is the delay in samples (can be fractional).
    /// Values are clamped to the valid range.
    #[inline]
    #[must_use]
    pub fn read(&self, delay: f32) -> f32 {
        let delay_clamped = delay.clamp(0.0, (self.max_delay_samples - 1) as f32);
        let delay_floor = delay_clamped.floor();
        let frac = delay_clamped - delay_floor;

        let read_pos_0 = (self.write_pos as isize - delay_floor as isize - 1)
            .rem_euclid(self.max_delay_samples as isize) as usize;
        let read_pos_1 = if read_pos_0 == 0 {
            self.max_delay_samples - 1
        } else {
            read_pos_0 - 1
        };

        self.buffer[read_pos_0] * (1.0 - frac) + self.buffer[read_pos_1] * frac
    }

    /// Clear the delay line.
    pub fn clear(&mut self) {
        self.buffer.fill(0.0);
    }
}

/// Feedback comb filter.
///
/// y\[n\] = x\[n\] + feedback * y\[n - delay\]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombFilter {
    /// The delay line.
    delay_line: DelayLine,
    /// Feedback coefficient (-1.0 to 1.0 for stability).
    pub feedback: f32,
    /// Delay time in samples.
    pub delay_samples: f32,
}

impl CombFilter {
    /// Create a new comb filter.
    ///
    /// `feedback` should be in (-1.0, 1.0) for stability.
    #[must_use]
    pub fn new(delay_samples: usize, feedback: f32) -> Self {
        Self {
            delay_line: DelayLine::new(delay_samples),
            feedback: feedback.clamp(-0.999, 0.999),
            delay_samples: delay_samples as f32,
        }
    }

    /// Process a single sample.
    #[inline]
    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let delayed = self.delay_line.read(self.delay_samples);
        let output = input + self.feedback * crate::flush_denormal(delayed);
        self.delay_line.write(output);
        output
    }
}

/// Schroeder allpass delay for use in reverb and phaser networks.
///
/// Single-buffer implementation: stores `x\[n\] + g * y\[n\]` in the buffer,
/// halving memory usage compared to a dual-buffer design.
///
/// y\[n\] = -g * x\[n\] + x\[n - D\] + g * y\[n - D\]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllpassDelay {
    /// Single delay buffer storing the combined signal.
    delay_line: DelayLine,
    /// Allpass coefficient (clamped to (-1, 1) for stability).
    coefficient: f32,
    /// Delay time in samples.
    delay_samples: f32,
}

impl AllpassDelay {
    /// Create a new allpass delay.
    #[must_use]
    pub fn new(delay_samples: usize, coefficient: f32) -> Self {
        Self {
            delay_line: DelayLine::new(delay_samples),
            coefficient: coefficient.clamp(-0.999, 0.999),
            delay_samples: delay_samples as f32,
        }
    }

    /// Returns the allpass coefficient.
    #[inline]
    #[must_use]
    pub fn coefficient(&self) -> f32 {
        self.coefficient
    }

    /// Returns the delay time in samples.
    #[inline]
    #[must_use]
    pub fn delay_samples(&self) -> f32 {
        self.delay_samples
    }

    /// Process a single sample through the Schroeder allpass.
    #[inline]
    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let delayed = self.delay_line.read(self.delay_samples);
        let output = -self.coefficient * input + delayed;
        // Store x[n] + g * y[n] in the buffer
        self.delay_line.write(input + self.coefficient * output);
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delay_line_basic() {
        let mut dl = DelayLine::new(10);
        dl.write(1.0);
        // Reading with delay 0 should return the just-written sample
        let val = dl.read(0.0);
        assert!((val - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_delay_line_delayed() {
        let mut dl = DelayLine::new(10);
        dl.write(1.0);
        for _ in 0..5 {
            dl.write(0.0);
        }
        let val = dl.read(5.0);
        assert!(
            (val - 1.0).abs() < f32::EPSILON,
            "expected 1.0 at 5 sample delay, got {val}"
        );
    }

    #[test]
    fn test_delay_fractional() {
        let mut dl = DelayLine::new(10);
        dl.write(0.0);
        dl.write(1.0);
        // Fractional delay between samples
        let val = dl.read(0.5);
        assert!(
            val > 0.0 && val < 1.0,
            "fractional delay should interpolate, got {val}"
        );
    }

    #[test]
    fn test_comb_filter() {
        let mut comb = CombFilter::new(100, 0.5);
        let out = comb.process_sample(1.0);
        assert!(out.is_finite());
    }

    #[test]
    fn test_allpass_delay() {
        let mut ap = AllpassDelay::new(100, 0.5);
        let out = ap.process_sample(1.0);
        assert!(out.is_finite());
    }

    #[test]
    fn test_serde_roundtrip() {
        let dl = DelayLine::new(100);
        let json = serde_json::to_string(&dl).unwrap();
        let back: DelayLine = serde_json::from_str(&json).unwrap();
        assert_eq!(dl.max_delay_samples, back.max_delay_samples);
    }
}
