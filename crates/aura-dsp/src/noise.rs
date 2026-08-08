//! Noise generators: white, pink (Voss-McCartney), and brown noise.

use serde::{Deserialize, Serialize};

/// Type of noise to generate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum NoiseType {
    /// Uniform random noise, flat spectrum.
    White,
    /// Pink noise (-3 dB/octave), Voss-McCartney algorithm.
    Pink,
    /// Brown noise (-6 dB/octave), integrated white noise.
    Brown,
}

/// Simple xorshift32 PRNG for deterministic noise.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Xorshift32 {
    state: u32,
}

impl Xorshift32 {
    fn new(seed: u32) -> Self {
        // Avoid zero state
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    /// Generate a random f32 in [-1.0, 1.0).
    #[inline]
    fn next_f32(&mut self) -> f32 {
        crate::dsp_util::xorshift32_signed_f32(&mut self.state)
    }
}

/// Number of octaves for Voss-McCartney pink noise.
const PINK_OCTAVES: usize = 16;

/// Noise generator with state for different noise types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoiseGenerator {
    /// The type of noise being generated.
    noise_type: NoiseType,
    /// PRNG state.
    rng: Xorshift32,
    /// Voss-McCartney octave values for pink noise.
    pink_octaves: [f32; PINK_OCTAVES],
    /// Counter for Voss-McCartney pink noise.
    pink_counter: u32,
    /// Running sum for pink noise.
    pink_running_sum: f32,
    /// Previous value for brown noise.
    brown_prev: f32,
}

impl NoiseGenerator {
    /// Create a new noise generator.
    ///
    /// # Arguments
    ///
    /// * `noise_type` - Type of noise to generate
    /// * `seed` - Random seed for deterministic output
    #[must_use]
    pub fn new(noise_type: NoiseType, seed: u32) -> Self {
        let mut rng = Xorshift32::new(seed);

        // Initialize pink noise octave values
        let mut pink_octaves = [0.0f32; PINK_OCTAVES];
        let mut pink_running_sum = 0.0f32;
        if noise_type == NoiseType::Pink {
            for octave in &mut pink_octaves {
                let val = rng.next_f32();
                *octave = val;
                pink_running_sum += val;
            }
        }

        Self {
            noise_type,
            rng,
            pink_octaves,
            pink_counter: 0,
            pink_running_sum,
            brown_prev: 0.0,
        }
    }

    /// Returns the type of noise being generated.
    #[inline]
    #[must_use]
    pub fn noise_type(&self) -> NoiseType {
        self.noise_type
    }

    /// Generate the next noise sample.
    #[inline]
    #[must_use]
    pub fn next_sample(&mut self) -> f32 {
        match self.noise_type {
            NoiseType::White => self.white_noise(),
            NoiseType::Pink => self.pink_noise(),
            NoiseType::Brown => self.brown_noise(),
        }
    }

    /// Generate white noise sample.
    #[inline]
    fn white_noise(&mut self) -> f32 {
        self.rng.next_f32()
    }

    /// Generate pink noise using Voss-McCartney algorithm.
    ///
    /// Uses a tree structure where each octave updates at half the rate
    /// of the previous, producing approximately -3 dB/octave rolloff.
    #[inline]
    fn pink_noise(&mut self) -> f32 {
        self.pink_counter = self.pink_counter.wrapping_add(1);

        // Determine which octaves to update based on trailing zeros
        let changed_bits = self.pink_counter ^ self.pink_counter.wrapping_sub(1);

        for i in 0..PINK_OCTAVES {
            if changed_bits & (1 << i) != 0 {
                self.pink_running_sum -= self.pink_octaves[i];
                let new_val = self.rng.next_f32();
                self.pink_octaves[i] = new_val;
                self.pink_running_sum += new_val;
            }
        }

        // Add white noise component and normalize
        let white = self.rng.next_f32();
        (self.pink_running_sum + white) / (PINK_OCTAVES as f32 + 1.0)
    }

    /// Generate brown noise (integrated white noise).
    #[inline]
    fn brown_noise(&mut self) -> f32 {
        let white = self.rng.next_f32();
        self.brown_prev += white * 0.02;
        self.brown_prev = self.brown_prev.clamp(-1.0, 1.0);
        // Apply leaky integrator to prevent DC drift
        self.brown_prev *= 0.999;
        self.brown_prev
    }

    /// Fill a buffer with noise samples.
    #[inline]
    pub fn fill_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.next_sample();
        }
    }
}

/// Generate a single white noise sample from a seed.
///
/// For convenience when you don't need persistent state.
#[must_use]
pub fn white_noise_sample(seed: &mut u32) -> f32 {
    crate::dsp_util::xorshift32_signed_f32(seed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_white_noise_range() {
        let mut ngen = NoiseGenerator::new(NoiseType::White, 42);
        for _ in 0..10000 {
            let s = ngen.next_sample();
            assert!((-1.0..=1.0).contains(&s), "white noise out of range: {s}");
        }
    }

    #[test]
    fn test_pink_noise_range() {
        let mut ngen = NoiseGenerator::new(NoiseType::Pink, 42);
        for _ in 0..10000 {
            let s = ngen.next_sample();
            assert!((-2.0..=2.0).contains(&s), "pink noise out of range: {s}");
        }
    }

    #[test]
    fn test_brown_noise_range() {
        let mut ngen = NoiseGenerator::new(NoiseType::Brown, 42);
        for _ in 0..10000 {
            let s = ngen.next_sample();
            assert!((-1.0..=1.0).contains(&s), "brown noise out of range: {s}");
        }
    }

    #[test]
    fn test_deterministic() {
        let mut ngen1 = NoiseGenerator::new(NoiseType::White, 42);
        let mut ngen2 = NoiseGenerator::new(NoiseType::White, 42);
        for _ in 0..100 {
            assert!((ngen1.next_sample() - ngen2.next_sample()).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn test_fill_buffer() {
        let mut ngen = NoiseGenerator::new(NoiseType::White, 42);
        let mut buf = [0.0f32; 256];
        ngen.fill_buffer(&mut buf);
        assert!(buf.iter().any(|&s| s != 0.0));
    }

    #[test]
    fn test_serde_roundtrip() {
        let ngen = NoiseGenerator::new(NoiseType::Pink, 123);
        let json = serde_json::to_string(&ngen).unwrap();
        let back: NoiseGenerator = serde_json::from_str(&json).unwrap();
        assert_eq!(ngen.noise_type(), back.noise_type());
    }
}
