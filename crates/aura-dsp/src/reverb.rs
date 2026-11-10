//! Algorithmic reverb based on the Schroeder topology.
//!
//! Implements a classic reverb with 4 parallel comb filters feeding into
//! 2 series allpass filters. Supports pre-delay, decay time, damping,
//! and stereo width.

use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::delay::DelayLine;
use crate::error::{self, Result};

/// Damped comb filter for reverb (comb + one-pole lowpass in feedback).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DampedComb {
    delay: DelayLine,
    delay_samples: f32,
    feedback: f32,
    damp1: f32,
    damp2: f32,
    #[serde(skip)]
    filter_state: f32,
}

impl DampedComb {
    fn new(delay_samples: usize, feedback: f32, damping: f32) -> Self {
        Self {
            delay: DelayLine::new(delay_samples),
            delay_samples: delay_samples as f32,
            feedback,
            damp1: damping,
            damp2: 1.0 - damping,
            filter_state: 0.0,
        }
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.delay.read(self.delay_samples);
        // One-pole lowpass damping in feedback path
        self.filter_state =
            crate::flush_denormal(delayed * self.damp2 + self.filter_state * self.damp1);
        self.delay.write(input + self.filter_state * self.feedback);
        delayed
    }

    #[inline]
    fn set_feedback(&mut self, feedback: f32) {
        self.feedback = feedback;
    }

    #[inline]
    fn set_damping(&mut self, damping: f32) {
        self.damp1 = damping;
        self.damp2 = 1.0 - damping;
    }
}

/// Simple allpass for reverb diffusion.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReverbAllpass {
    delay: DelayLine,
    delay_samples: f32,
    feedback: f32,
}

impl ReverbAllpass {
    fn new(delay_samples: usize, feedback: f32) -> Self {
        Self {
            delay: DelayLine::new(delay_samples),
            delay_samples: delay_samples as f32,
            feedback,
        }
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.delay.read(self.delay_samples);
        let output = -self.feedback * input + delayed;
        self.delay
            .write(input + self.feedback * crate::flush_denormal(output));
        output
    }
}

/// Schroeder algorithmic reverb.
///
/// Classic topology: 4 parallel comb filters → 2 series allpass filters.
/// Supports pre-delay, decay time, high-frequency damping, wet/dry mix,
/// and stereo width.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reverb {
    /// Pre-delay line.
    pre_delay: DelayLine,
    /// Pre-delay time in samples.
    pre_delay_samples: f32,
    /// Left-channel comb filters.
    combs_l: [DampedComb; 4],
    /// Right-channel comb filters (slightly offset for stereo).
    combs_r: [DampedComb; 4],
    /// Left-channel allpass filters.
    allpasses_l: [ReverbAllpass; 2],
    /// Right-channel allpass filters.
    allpasses_r: [ReverbAllpass; 2],
    /// Wet/dry mix (0.0 = dry, 1.0 = wet).
    pub mix: f32,
    /// Stereo width (0.0 = mono, 1.0 = full stereo).
    pub width: f32,
    /// Sample rate.
    sample_rate: f32,
}

// Prime delay lengths (in samples at 44100 Hz) for minimal modal clustering.
// All values are prime and mutually coprime for optimal diffusion.
const COMB_LENGTHS: [usize; 4] = [1117, 1187, 1277, 1361];
const ALLPASS_LENGTHS: [usize; 2] = [557, 443];
// Stereo offset (samples added to right channel for decorrelation)
const STEREO_OFFSET: usize = 23;

impl Reverb {
    /// Create a new reverb.
    ///
    /// # Arguments
    ///
    /// * `decay` - Decay time (0.0 to 1.0, controls feedback)
    /// * `damping` - High-frequency damping (0.0 = bright, 1.0 = dark)
    /// * `pre_delay_ms` - Pre-delay in milliseconds
    /// * `mix` - Wet/dry mix (0.0 to 1.0)
    /// * `sample_rate` - Sample rate in Hz
    ///
    /// # Errors
    ///
    /// Returns error if `sample_rate` is invalid.
    pub fn new(
        decay: f32,
        damping: f32,
        pre_delay_ms: f32,
        mix: f32,
        sample_rate: f32,
    ) -> Result<Self> {
        if let Some(e) = error::validate_sample_rate(sample_rate) {
            return Err(e);
        }

        debug!(
            decay,
            damping, pre_delay_ms, mix, sample_rate, "reverb created"
        );
        let scale = sample_rate / 44100.0;
        let feedback = decay.clamp(0.0, 0.99);
        let damp = damping.clamp(0.0, 1.0);
        let pre_samples = (pre_delay_ms * sample_rate / 1000.0).max(0.0);

        let make_comb = |base: usize, offset: usize| -> DampedComb {
            let len = ((base + offset) as f32 * scale) as usize;
            DampedComb::new(len.max(1), feedback, damp)
        };

        let make_ap = |base: usize, offset: usize| -> ReverbAllpass {
            let len = ((base + offset) as f32 * scale) as usize;
            ReverbAllpass::new(len.max(1), 0.5)
        };

        Ok(Self {
            pre_delay: DelayLine::new((pre_samples as usize).max(1)),
            pre_delay_samples: pre_samples,
            combs_l: [
                make_comb(COMB_LENGTHS[0], 0),
                make_comb(COMB_LENGTHS[1], 0),
                make_comb(COMB_LENGTHS[2], 0),
                make_comb(COMB_LENGTHS[3], 0),
            ],
            combs_r: [
                make_comb(COMB_LENGTHS[0], STEREO_OFFSET),
                make_comb(COMB_LENGTHS[1], STEREO_OFFSET),
                make_comb(COMB_LENGTHS[2], STEREO_OFFSET),
                make_comb(COMB_LENGTHS[3], STEREO_OFFSET),
            ],
            allpasses_l: [
                make_ap(ALLPASS_LENGTHS[0], 0),
                make_ap(ALLPASS_LENGTHS[1], 0),
            ],
            allpasses_r: [
                make_ap(ALLPASS_LENGTHS[0], STEREO_OFFSET),
                make_ap(ALLPASS_LENGTHS[1], STEREO_OFFSET),
            ],
            mix: mix.clamp(0.0, 1.0),
            width: 1.0,
            sample_rate,
        })
    }

    /// RT-safe update of comb feedback from decay in `[0, 1]` (clamped to `0..0.99`).
    ///
    /// Non-finite values are ignored (previous feedback kept). Does not allocate or
    /// resize delay lines — safe to call from the audio thread when params move.
    #[inline]
    pub fn set_decay(&mut self, decay: f32) {
        if !decay.is_finite() {
            return;
        }
        let feedback = decay.clamp(0.0, 0.99);
        for comb in self.combs_l.iter_mut().chain(self.combs_r.iter_mut()) {
            comb.set_feedback(feedback);
        }
    }

    /// RT-safe update of comb high-frequency damping in `[0, 1]`.
    ///
    /// `0.0` = bright, `1.0` = dark (same mapping as [`Reverb::new`]). Non-finite
    /// values are ignored. Does not allocate or resize delay lines.
    #[inline]
    pub fn set_damping(&mut self, damping: f32) {
        if !damping.is_finite() {
            return;
        }
        let damp = damping.clamp(0.0, 1.0);
        for comb in self.combs_l.iter_mut().chain(self.combs_r.iter_mut()) {
            comb.set_damping(damp);
        }
    }

    /// Process a mono input and return stereo output (left, right).
    #[inline]
    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> (f32, f32) {
        // Pre-delay
        self.pre_delay.write(input);
        let delayed = self.pre_delay.read(self.pre_delay_samples);

        // Parallel comb filters
        let mut wet_l = 0.0f32;
        let mut wet_r = 0.0f32;
        for comb in &mut self.combs_l {
            wet_l += comb.process(delayed);
        }
        for comb in &mut self.combs_r {
            wet_r += comb.process(delayed);
        }
        wet_l *= 0.25; // normalize by comb count
        wet_r *= 0.25;

        // Series allpass filters
        for ap in &mut self.allpasses_l {
            wet_l = ap.process(wet_l);
        }
        for ap in &mut self.allpasses_r {
            wet_r = ap.process(wet_r);
        }

        // Stereo width: blend left/right
        let w = self.width;
        let out_l = wet_l * (0.5 + 0.5 * w) + wet_r * (0.5 - 0.5 * w);
        let out_r = wet_r * (0.5 + 0.5 * w) + wet_l * (0.5 - 0.5 * w);

        // Wet/dry mix
        let dry = 1.0 - self.mix;
        (
            input * dry + out_l * self.mix,
            input * dry + out_r * self.mix,
        )
    }

    /// Process a mono buffer into stereo output buffers.
    pub fn process_buffer(&mut self, input: &[f32], left: &mut [f32], right: &mut [f32]) {
        for (i, &s) in input.iter().enumerate() {
            let (l, r) = self.process_sample(s);
            if i < left.len() {
                left[i] = l;
            }
            if i < right.len() {
                right[i] = r;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reverb_produces_output() {
        let mut rev = Reverb::new(0.8, 0.3, 10.0, 1.0, 44100.0).unwrap();
        // Feed an impulse
        let (l, r) = rev.process_sample(1.0);
        assert!(l.is_finite());
        assert!(r.is_finite());
        // After impulse, reverb tail should still produce output
        let mut has_tail = false;
        for _ in 0..10000 {
            let (l, _r) = rev.process_sample(0.0);
            if l.abs() > 0.001 {
                has_tail = true;
                break;
            }
        }
        assert!(has_tail, "reverb should produce a tail after impulse");
    }

    #[test]
    fn test_reverb_stereo_differs() {
        let mut rev = Reverb::new(0.8, 0.3, 10.0, 1.0, 44100.0).unwrap();
        rev.width = 1.0;
        // Feed impulse and collect stereo output
        let _ = rev.process_sample(1.0);
        let mut diff_found = false;
        for _ in 0..5000 {
            let (l, r) = rev.process_sample(0.0);
            if (l - r).abs() > 0.001 {
                diff_found = true;
                break;
            }
        }
        assert!(diff_found, "stereo reverb should have L/R differences");
    }

    #[test]
    fn test_reverb_dry_passthrough() {
        let mut rev = Reverb::new(0.5, 0.5, 0.0, 0.0, 44100.0).unwrap();
        let (l, r) = rev.process_sample(0.7);
        assert!((l - 0.7).abs() < 0.01, "mix=0 should pass dry signal");
        assert!((r - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_reverb_no_blow_up() {
        let mut rev = Reverb::new(0.99, 0.0, 0.0, 1.0, 44100.0).unwrap();
        let _ = rev.process_sample(1.0);
        for _ in 0..100_000 {
            let (l, r) = rev.process_sample(0.0);
            assert!(l.abs() < 10.0, "reverb should not blow up: {l}");
            assert!(r.abs() < 10.0, "reverb should not blow up: {r}");
        }
    }

    #[test]
    fn test_serde_roundtrip() {
        let rev = Reverb::new(0.7, 0.4, 15.0, 0.5, 44100.0).unwrap();
        let json = serde_json::to_string(&rev).unwrap();
        let back: Reverb = serde_json::from_str(&json).unwrap();
        assert!((rev.mix - back.mix).abs() < f32::EPSILON);
    }

    fn tail_energy(rev: &mut Reverb, samples: usize) -> f32 {
        let mut e = 0.0f32;
        for _ in 0..samples {
            let (l, r) = rev.process_sample(0.0);
            e += l * l + r * r;
        }
        e
    }

    #[test]
    fn set_decay_changes_tail_energy() {
        let mut high = Reverb::new(0.5, 0.2, 0.0, 1.0, 44100.0).unwrap();
        let mut low = Reverb::new(0.5, 0.2, 0.0, 1.0, 44100.0).unwrap();
        high.set_decay(0.95);
        low.set_decay(0.1);
        let _ = high.process_sample(1.0);
        let _ = low.process_sample(1.0);
        // Skip pre-delay/comb fill; compare sustained tail.
        let e_high = tail_energy(&mut high, 8_000);
        let e_low = tail_energy(&mut low, 8_000);
        assert!(
            e_high > e_low * 2.0,
            "higher decay must keep more tail energy: high={e_high} low={e_low}"
        );
    }

    #[test]
    fn set_damping_keeps_output_finite() {
        let mut rev = Reverb::new(0.8, 0.3, 0.0, 1.0, 44100.0).unwrap();
        rev.set_damping(0.0);
        rev.set_damping(1.0);
        rev.set_damping(f32::NAN);
        rev.set_damping(f32::INFINITY);
        let _ = rev.process_sample(1.0);
        for _ in 0..2_000 {
            let (l, r) = rev.process_sample(0.0);
            assert!(
                l.is_finite() && r.is_finite(),
                "damping extremes must stay finite"
            );
        }
    }

    #[test]
    fn set_decay_nan_is_noop() {
        let mut a = Reverb::new(0.8, 0.2, 0.0, 1.0, 44100.0).unwrap();
        let mut b = Reverb::new(0.8, 0.2, 0.0, 1.0, 44100.0).unwrap();
        a.set_decay(0.8);
        b.set_decay(0.8);
        b.set_decay(f32::NAN);
        let _ = a.process_sample(1.0);
        let _ = b.process_sample(1.0);
        let ea = tail_energy(&mut a, 4_000);
        let eb = tail_energy(&mut b, 4_000);
        assert!(
            (ea - eb).abs() <= ea * 1e-5 + 1e-12,
            "NaN decay must not change tail vs previous finite setting: a={ea} b={eb}"
        );
    }
}
