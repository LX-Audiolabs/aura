//! ADSR envelope generator and multi-stage envelopes.
//!
//! Provides standard Attack-Decay-Sustain-Release envelopes with linear
//! segments, plus a flexible multi-stage envelope for arbitrary shapes.

use serde::{Deserialize, Serialize};

#[cfg(feature = "synthesis")]
use hisab::Vec3;

use crate::error::{NaadError, Result};

/// Envelope state machine stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum EnvelopeState {
    /// Envelope is inactive (output = 0).
    Idle,
    /// Attack phase (rising from 0 to 1).
    Attack,
    /// Decay phase (falling from 1 to sustain level).
    Decay,
    /// Sustain phase (holding at sustain level).
    Sustain,
    /// Release phase (falling from current to 0).
    Release,
}

/// ADSR envelope generator with linear segments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Adsr {
    /// Attack time in seconds.
    ///
    /// Note: modifying this directly bypasses validation. Use the constructor
    /// for guaranteed-valid values.
    pub attack_time: f32,
    /// Decay time in seconds.
    ///
    /// Note: modifying this directly bypasses validation. Use the constructor
    /// for guaranteed-valid values.
    pub decay_time: f32,
    /// Sustain level (0.0 to 1.0).
    ///
    /// Note: modifying this directly bypasses validation. Use the constructor
    /// for guaranteed-valid values.
    pub sustain_level: f32,
    /// Release time in seconds.
    ///
    /// Note: modifying this directly bypasses validation. Use the constructor
    /// for guaranteed-valid values.
    pub release_time: f32,
    /// Sample rate in Hz.
    sample_rate: f32,
    /// Current envelope state.
    state: EnvelopeState,
    /// Current output value.
    current_value: f32,
    /// Value at the start of the release phase.
    release_start_value: f32,
    /// Time spent in the current stage (in samples).
    stage_samples: f32,
}

impl Adsr {
    /// Create a new ADSR envelope.
    ///
    /// All times are in seconds. Sustain level is 0.0 to 1.0.
    ///
    /// # Errors
    ///
    /// Returns error if any time is negative or sustain is out of range.
    pub fn new(attack: f32, decay: f32, sustain: f32, release: f32) -> Result<Self> {
        Self::with_sample_rate(attack, decay, sustain, release, 44100.0)
    }

    /// Create a new ADSR envelope with an explicit sample rate.
    ///
    /// # Errors
    ///
    /// Returns error if any time is negative, sustain is out of range,
    /// or sample_rate is invalid.
    pub fn with_sample_rate(
        attack: f32,
        decay: f32,
        sustain: f32,
        release: f32,
        sample_rate: f32,
    ) -> Result<Self> {
        if attack < 0.0 {
            return Err(NaadError::InvalidParameter {
                name: "attack".to_string(),
                reason: "must be >= 0".to_string(),
            });
        }
        if decay < 0.0 {
            return Err(NaadError::InvalidParameter {
                name: "decay".to_string(),
                reason: "must be >= 0".to_string(),
            });
        }
        if !(0.0..=1.0).contains(&sustain) {
            return Err(NaadError::InvalidParameter {
                name: "sustain".to_string(),
                reason: "must be between 0.0 and 1.0".to_string(),
            });
        }
        if release < 0.0 {
            return Err(NaadError::InvalidParameter {
                name: "release".to_string(),
                reason: "must be >= 0".to_string(),
            });
        }
        if sample_rate <= 0.0 || !sample_rate.is_finite() {
            return Err(NaadError::InvalidSampleRate { sample_rate });
        }

        Ok(Self {
            attack_time: attack,
            decay_time: decay,
            sustain_level: sustain,
            release_time: release,
            sample_rate,
            state: EnvelopeState::Idle,
            current_value: 0.0,
            release_start_value: 0.0,
            stage_samples: 0.0,
        })
    }

    /// Returns the current envelope state.
    #[inline]
    #[must_use]
    pub fn state(&self) -> EnvelopeState {
        self.state
    }

    /// Trigger the envelope (note on).
    pub fn gate_on(&mut self) {
        self.state = EnvelopeState::Attack;
        self.stage_samples = 0.0;
    }

    /// Release the envelope (note off).
    pub fn gate_off(&mut self) {
        if self.state != EnvelopeState::Idle {
            self.release_start_value = self.current_value;
            self.state = EnvelopeState::Release;
            self.stage_samples = 0.0;
        }
    }

    /// Generate the next envelope value.
    ///
    /// Returns a value between 0.0 and 1.0.
    #[inline]
    #[must_use]
    pub fn next_value(&mut self) -> f32 {
        let sr = self.sample_rate;
        match self.state {
            EnvelopeState::Idle => {
                self.current_value = 0.0;
            }
            EnvelopeState::Attack => {
                let attack_samples = self.attack_time * sr;
                if attack_samples <= 0.0 {
                    self.current_value = 1.0;
                    self.state = EnvelopeState::Decay;
                    self.stage_samples = 0.0;
                } else {
                    self.current_value = self.stage_samples / attack_samples;
                    self.stage_samples += 1.0;
                    if self.current_value >= 1.0 {
                        self.current_value = 1.0;
                        self.state = EnvelopeState::Decay;
                        self.stage_samples = 0.0;
                    }
                }
            }
            EnvelopeState::Decay => {
                let decay_samples = self.decay_time * sr;
                if decay_samples <= 0.0 {
                    self.current_value = self.sustain_level;
                    self.state = EnvelopeState::Sustain;
                } else {
                    let progress = self.stage_samples / decay_samples;
                    self.current_value = 1.0 + (self.sustain_level - 1.0) * progress;
                    self.stage_samples += 1.0;
                    if self.current_value <= self.sustain_level {
                        self.current_value = self.sustain_level;
                        self.state = EnvelopeState::Sustain;
                    }
                }
            }
            EnvelopeState::Sustain => {
                self.current_value = self.sustain_level;
            }
            EnvelopeState::Release => {
                let release_samples = self.release_time * sr;
                if release_samples <= 0.0 {
                    self.current_value = 0.0;
                    self.state = EnvelopeState::Idle;
                } else {
                    let progress = self.stage_samples / release_samples;
                    self.current_value = self.release_start_value * (1.0 - progress);
                    self.stage_samples += 1.0;
                    if self.current_value <= 0.0 {
                        self.current_value = 0.0;
                        self.state = EnvelopeState::Idle;
                    }
                }
            }
        }

        self.current_value
    }

    /// Check if the envelope is active (not idle).
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.state != EnvelopeState::Idle
    }
}

/// A single segment in a multi-stage envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvelopeSegment {
    /// Target level for this segment (0.0 to 1.0).
    pub target: f32,
    /// Duration in seconds.
    pub duration: f32,
}

/// Multi-stage envelope with arbitrary segments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiStageEnvelope {
    /// The segments of the envelope.
    pub segments: Vec<EnvelopeSegment>,
    /// Sample rate in Hz.
    sample_rate: f32,
    /// Current segment index.
    current_segment: usize,
    /// Current output value.
    current_value: f32,
    /// Start value of current segment.
    segment_start_value: f32,
    /// Time spent in current segment (in samples).
    stage_samples: f32,
    /// Whether the envelope is active.
    active: bool,
}

impl MultiStageEnvelope {
    /// Create a new multi-stage envelope (defaults to 44100 Hz sample rate).
    ///
    /// # Errors
    ///
    /// Returns error if segments is empty.
    pub fn new(segments: Vec<EnvelopeSegment>) -> Result<Self> {
        Self::with_sample_rate(segments, 44100.0)
    }

    /// Create a new multi-stage envelope with an explicit sample rate.
    ///
    /// # Errors
    ///
    /// Returns error if segments is empty or sample_rate is invalid.
    pub fn with_sample_rate(segments: Vec<EnvelopeSegment>, sample_rate: f32) -> Result<Self> {
        if segments.is_empty() {
            return Err(NaadError::InvalidParameter {
                name: "segments".to_string(),
                reason: "must have at least one segment".to_string(),
            });
        }
        if sample_rate <= 0.0 || !sample_rate.is_finite() {
            return Err(NaadError::InvalidSampleRate { sample_rate });
        }

        Ok(Self {
            segments,
            sample_rate,
            current_segment: 0,
            current_value: 0.0,
            segment_start_value: 0.0,
            stage_samples: 0.0,
            active: false,
        })
    }

    /// Start the envelope.
    pub fn trigger(&mut self) {
        self.current_segment = 0;
        self.current_value = 0.0;
        self.segment_start_value = 0.0;
        self.stage_samples = 0.0;
        self.active = true;
    }

    /// Generate the next envelope value.
    #[inline]
    #[must_use]
    pub fn next_value(&mut self) -> f32 {
        if !self.active {
            return 0.0;
        }

        if self.current_segment >= self.segments.len() {
            self.active = false;
            return 0.0;
        }

        let seg = &self.segments[self.current_segment];
        let seg_samples = seg.duration * self.sample_rate;

        if seg_samples <= 0.0 {
            self.current_value = seg.target;
            self.segment_start_value = self.current_value;
            self.current_segment += 1;
            self.stage_samples = 0.0;
        } else {
            let progress = (self.stage_samples / seg_samples).min(1.0);
            self.current_value =
                self.segment_start_value + (seg.target - self.segment_start_value) * progress;
            self.stage_samples += 1.0;

            if self.stage_samples >= seg_samples {
                self.current_value = seg.target;
                self.segment_start_value = self.current_value;
                self.current_segment += 1;
                self.stage_samples = 0.0;
            }
        }

        self.current_value
    }

    /// Check if the envelope is active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }
}

/// One control point of a [`CatmullRomEnvelope`].
#[cfg(feature = "synthesis")]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EnvelopePoint {
    /// Seconds from envelope trigger.
    pub time: f32,
    /// Target value at this time.
    pub value: f32,
}

/// Smooth envelope curve interpolating user-placed control points with Catmull-Rom splines.
///
/// Where [`MultiStageEnvelope`] connects targets with linear segments,
/// `CatmullRomEnvelope` connects them with C¹-continuous cubic splines via
/// `hisab::calc::splines::catmull_rom` — no kinks at control points,
/// well-suited to organic / vocal-style amplitude shapes that linear ADSRs
/// can't capture without dozens of segments.
///
/// At the endpoints the curve is clamped (the phantom outer points mirror
/// the first/last actual points) so it doesn't overshoot before t=0 or
/// after the last control point.
///
/// Behind the `synthesis` feature (uses hisab).
#[cfg(feature = "synthesis")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatmullRomEnvelope {
    /// Control points, ordered by `time`.
    points: Vec<EnvelopePoint>,
    /// Sample rate in Hz.
    sample_rate: f32,
    /// Elapsed samples since trigger.
    elapsed_samples: f32,
    /// Whether the envelope is currently producing values.
    active: bool,
}

#[cfg(feature = "synthesis")]
impl CatmullRomEnvelope {
    /// Build a Catmull-Rom envelope from a sequence of control points.
    ///
    /// `points` must contain at least 2 entries with strictly increasing
    /// `time` values; the first should typically be at `time = 0.0`.
    ///
    /// # Errors
    ///
    /// Returns [`NaadError::InvalidParameter`] if fewer than 2 points are
    /// supplied or times are not strictly increasing, or
    /// [`NaadError::InvalidSampleRate`] for a bad sample rate.
    pub fn new(points: Vec<EnvelopePoint>, sample_rate: f32) -> Result<Self> {
        if points.len() < 2 {
            return Err(NaadError::InvalidParameter {
                name: "points".to_string(),
                reason: "need at least 2 control points".to_string(),
            });
        }
        for w in points.windows(2) {
            if w[1].time <= w[0].time {
                return Err(NaadError::InvalidParameter {
                    name: "points[*].time".to_string(),
                    reason: "control point times must be strictly increasing".to_string(),
                });
            }
        }
        if sample_rate <= 0.0 || !sample_rate.is_finite() {
            return Err(NaadError::InvalidSampleRate { sample_rate });
        }
        Ok(Self {
            points,
            sample_rate,
            elapsed_samples: 0.0,
            active: false,
        })
    }

    /// Start the envelope from `t = 0`.
    pub fn trigger(&mut self) {
        self.elapsed_samples = 0.0;
        self.active = true;
    }

    /// Stop and reset the envelope (next `next_value` returns 0).
    pub fn release(&mut self) {
        self.active = false;
    }

    /// Generate the next envelope sample.
    ///
    /// Returns the spline value at the current elapsed time. After the
    /// last control point, returns the last point's value and marks the
    /// envelope inactive.
    #[inline]
    #[must_use]
    pub fn next_value(&mut self) -> f32 {
        if !self.active {
            return 0.0;
        }

        let t = self.elapsed_samples / self.sample_rate;
        self.elapsed_samples += 1.0;

        let n = self.points.len();
        let last_time = self.points[n - 1].time;

        if t >= last_time {
            self.active = false;
            return self.points[n - 1].value;
        }
        if t <= self.points[0].time {
            return self.points[0].value;
        }

        // Locate the segment [i, i+1] containing t. Linear scan is fine —
        // envelopes typically have <32 control points.
        let mut i = 0usize;
        for k in 0..(n - 1) {
            if t < self.points[k + 1].time {
                i = k;
                break;
            }
        }

        let p1 = self.points[i];
        let p2 = self.points[i + 1];
        // Phantom endpoints clamp the curve at the boundaries.
        let p0 = if i == 0 { p1 } else { self.points[i - 1] };
        let p3 = if i + 2 >= n { p2 } else { self.points[i + 2] };

        let u = ((t - p1.time) / (p2.time - p1.time)).clamp(0.0, 1.0);

        // Catmull-Rom on scalars: lift each value into Vec3.x, run hisab's
        // catmull_rom, take .x back. Vec3 indirection is the price for
        // staying on the canonical hisab implementation.
        let lifted = hisab::calc::catmull_rom(
            Vec3::new(p0.value, 0.0, 0.0),
            Vec3::new(p1.value, 0.0, 0.0),
            Vec3::new(p2.value, 0.0, 0.0),
            Vec3::new(p3.value, 0.0, 0.0),
            u,
        );
        lifted.x
    }

    /// Check if the envelope is currently producing values.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Number of control points.
    #[must_use]
    pub fn num_points(&self) -> usize {
        self.points.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adsr_basic() {
        let mut env = Adsr::new(0.01, 0.01, 0.5, 0.01).unwrap();
        assert!(!env.is_active());
        env.gate_on();
        assert!(env.is_active());
    }

    #[test]
    fn test_adsr_sustain_holds() {
        let mut env = Adsr::new(0.001, 0.001, 0.7, 0.01).unwrap();
        env.gate_on();
        // Run through attack + decay
        for _ in 0..1000 {
            let _ = env.next_value();
        }
        // Should be at sustain level
        let val = env.next_value();
        assert!(
            (val - 0.7).abs() < 0.01,
            "sustain should hold at 0.7, got {val}"
        );
    }

    #[test]
    fn test_adsr_release_to_zero() {
        let mut env = Adsr::new(0.0, 0.0, 1.0, 0.01).unwrap();
        env.gate_on();
        let _ = env.next_value();
        env.gate_off();
        for _ in 0..2000 {
            let _ = env.next_value();
        }
        assert!(!env.is_active());
    }

    #[test]
    fn test_invalid_params() {
        assert!(Adsr::new(-1.0, 0.0, 0.5, 0.0).is_err());
        assert!(Adsr::new(0.0, 0.0, 1.5, 0.0).is_err());
        assert!(Adsr::new(0.0, 0.0, -0.1, 0.0).is_err());
    }

    #[test]
    fn test_multi_stage() {
        let segments = vec![
            EnvelopeSegment {
                target: 1.0,
                duration: 0.01,
            },
            EnvelopeSegment {
                target: 0.5,
                duration: 0.01,
            },
            EnvelopeSegment {
                target: 0.0,
                duration: 0.01,
            },
        ];
        let mut env = MultiStageEnvelope::new(segments).unwrap();
        env.trigger();
        assert!(env.is_active());
        for _ in 0..5000 {
            let _ = env.next_value();
        }
        assert!(!env.is_active());
    }

    #[test]
    fn test_serde_roundtrip() {
        let env = Adsr::new(0.01, 0.1, 0.5, 0.2).unwrap();
        let json = serde_json::to_string(&env).unwrap();
        let back: Adsr = serde_json::from_str(&json).unwrap();
        assert!((env.attack_time - back.attack_time).abs() < f32::EPSILON);
        assert!((env.sustain_level - back.sustain_level).abs() < f32::EPSILON);
    }

    #[cfg(feature = "synthesis")]
    fn pts(spec: &[(f32, f32)]) -> Vec<EnvelopePoint> {
        spec.iter()
            .map(|&(t, v)| EnvelopePoint { time: t, value: v })
            .collect()
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_catmull_rom_passes_through_control_points() {
        let sr = 44100.0;
        let mut env =
            CatmullRomEnvelope::new(pts(&[(0.0, 0.0), (0.25, 1.0), (0.5, 0.3), (1.0, 0.0)]), sr)
                .unwrap();
        env.trigger();

        // Sample exactly at each control-point time and verify the spline
        // hits the target value (Catmull-Rom interpolates its anchors).
        let want_at = [(0, 0.0), (11_025, 1.0), (22_050, 0.3), (44_100, 0.0)];
        let mut last_idx = 0usize;
        let mut current = env.next_value();
        for (idx, target) in want_at {
            while last_idx < idx {
                current = env.next_value();
                last_idx += 1;
            }
            assert!(
                (current - target).abs() < 1e-3,
                "at sample {idx}: spline = {current}, expected {target}"
            );
        }
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_catmull_rom_smooth_no_kinks() {
        // C¹-continuity: the absolute first difference shouldn't jump
        // sharply across control-point boundaries (vs a linear envelope
        // which has visible kinks).
        let sr = 44100.0;
        let mut env =
            CatmullRomEnvelope::new(pts(&[(0.0, 0.0), (0.1, 1.0), (0.2, 0.0), (0.3, 0.5)]), sr)
                .unwrap();
        env.trigger();
        let n = (0.3 * sr) as usize;
        let buf: Vec<f32> = (0..n).map(|_| env.next_value()).collect();
        let max_diff = buf
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max);
        // For a 100ms spline rise from 0→1 at 44.1 kHz, max sample-to-sample
        // delta is well under 0.01 for a smooth Catmull-Rom curve.
        assert!(
            max_diff < 0.01,
            "Catmull-Rom envelope shouldn't have kinks; max |Δ| = {max_diff}"
        );
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_catmull_rom_deactivates_after_last_point() {
        let sr = 44100.0;
        let mut env =
            CatmullRomEnvelope::new(pts(&[(0.0, 0.0), (0.05, 0.7), (0.1, 0.3)]), sr).unwrap();
        env.trigger();
        // Crossing the last control point returns its value once, then
        // deactivates — subsequent calls return 0 like other envelopes.
        let last_idx = (0.1 * sr) as usize;
        for i in 0..=last_idx {
            let v = env.next_value();
            if i == last_idx {
                assert!(
                    (v - 0.3).abs() < 1e-3,
                    "at last control point: got {v}, expected 0.3"
                );
            }
        }
        assert!(
            !env.is_active(),
            "envelope should deactivate after last point"
        );
        // Past the end: behaves like MultiStageEnvelope (returns 0 when inactive).
        assert!(env.next_value().abs() < f32::EPSILON);
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_catmull_rom_invalid_inputs() {
        let sr = 44100.0;
        // <2 points
        assert!(CatmullRomEnvelope::new(pts(&[(0.0, 0.0)]), sr).is_err());
        // Non-monotone times
        assert!(CatmullRomEnvelope::new(pts(&[(0.0, 0.0), (0.5, 1.0), (0.5, 0.0)]), sr).is_err());
        // Bad sample rate
        assert!(CatmullRomEnvelope::new(pts(&[(0.0, 0.0), (0.1, 1.0)]), -1.0).is_err());
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_catmull_rom_serde_roundtrip() {
        let env = CatmullRomEnvelope::new(
            pts(&[(0.0, 0.0), (0.1, 1.0), (0.3, 0.5), (0.5, 0.0)]),
            48000.0,
        )
        .unwrap();
        let json = serde_json::to_string(&env).unwrap();
        let back: CatmullRomEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(env.num_points(), back.num_points());
    }
}
