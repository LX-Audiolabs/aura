//! Dynamics processors: compressor, limiter, and noise gate.
//!
//! All processors operate sample-by-sample for real-time use.

use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::dsp_util;

/// Smoothed level detector for dynamics processing.
///
/// Tracks the abs-value envelope of an input signal with separate attack
/// and release time constants. Renamed from `EnvelopeDetector` in 1.1.0
/// to disambiguate from the unrelated `EnvelopeState` in
/// [`crate::envelope`] — both prefixes shared "Envelope" for different
/// concepts (the dynamics envelope vs. the ADSR state machine).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelDetector {
    /// Current envelope value (linear).
    current: f32,
    /// Attack coefficient.
    attack_coeff: f32,
    /// Release coefficient.
    release_coeff: f32,
}

impl LevelDetector {
    /// Create a new envelope detector.
    ///
    /// `attack` and `release` are times in seconds.
    #[must_use]
    pub fn new(attack: f32, release: f32, sample_rate: f32) -> Self {
        Self {
            current: 0.0,
            attack_coeff: Self::time_to_coeff(attack, sample_rate),
            release_coeff: Self::time_to_coeff(release, sample_rate),
        }
    }

    fn time_to_coeff(time: f32, sample_rate: f32) -> f32 {
        if time <= 0.0 {
            1.0
        } else {
            1.0 - (-1.0 / (time * sample_rate)).exp()
        }
    }

    /// Process a sample and return the envelope level.
    #[inline]
    #[must_use]
    pub fn process(&mut self, input: f32) -> f32 {
        let level = if input.is_finite() { input.abs() } else { 0.0 };
        let coeff = if level > self.current {
            self.attack_coeff
        } else {
            self.release_coeff
        };
        self.current += coeff * (level - self.current);
        self.current = crate::flush_denormal(self.current);
        self.current
    }
}

/// Dynamics compressor with soft knee.
///
/// Reduces dynamic range by attenuating signals above a threshold.
/// Supports configurable ratio, attack, release, makeup gain, and knee width.
///
/// `threshold_db`, `knee_db`, and `makeup_db` are direct-read parameter
/// fields — modifying them takes effect on the next sample with no
/// validation. `ratio` is private because it is clamped to `>= 1.0` at
/// construction; use [`Self::set_ratio`] to enforce the same invariant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Compressor {
    /// Threshold in dB.
    pub threshold_db: f32,
    /// Compression ratio (e.g., 4.0 = 4:1). Clamped to `>= 1.0`.
    ratio: f32,
    /// Knee width in dB (0.0 = hard knee).
    pub knee_db: f32,
    /// Makeup gain in dB.
    pub makeup_db: f32,
    /// Envelope detector.
    detector: LevelDetector,
}

impl Compressor {
    /// Create a new compressor.
    ///
    /// # Arguments
    ///
    /// * `threshold_db` - Threshold in dB (e.g., -20.0)
    /// * `ratio` - Compression ratio (e.g., 4.0 for 4:1; clamped to `>= 1.0`)
    /// * `attack` - Attack time in seconds
    /// * `release` - Release time in seconds
    /// * `sample_rate` - Sample rate in Hz
    #[must_use]
    pub fn new(threshold_db: f32, ratio: f32, attack: f32, release: f32, sample_rate: f32) -> Self {
        debug!(threshold_db, ratio, attack, release, "compressor created");
        Self {
            threshold_db,
            ratio: ratio.max(1.0),
            knee_db: 0.0,
            makeup_db: 0.0,
            detector: LevelDetector::new(attack, release, sample_rate),
        }
    }

    /// Returns the current compression ratio.
    #[must_use]
    pub fn ratio(&self) -> f32 {
        self.ratio
    }

    /// Set the compression ratio. Clamped to `>= 1.0`.
    pub fn set_ratio(&mut self, ratio: f32) {
        self.ratio = ratio.max(1.0);
    }

    /// Hard-knee gain reduction (`knee_db == 0.0` fast path).
    ///
    /// Skips the soft-knee branch and the `(input_db - t).abs() > k*0.5`
    /// check — both are dead when the knee is zero. Inlined into
    /// [`Self::process_sample`] for the common hard-knee case.
    #[inline]
    fn compute_gain_db_hard(&self, input_db: f32) -> f32 {
        if input_db <= self.threshold_db {
            0.0
        } else {
            (self.threshold_db + (input_db - self.threshold_db) / self.ratio) - input_db
        }
    }

    /// Soft-knee gain reduction (`knee_db > 0.0`).
    ///
    /// Uses the quadratic interpolation between unity gain and full ratio
    /// across the knee width. Hard-knee callers hit [`Self::compute_gain_db_hard`]
    /// instead — `process_sample` dispatches once per sample.
    #[inline]
    fn compute_gain_db_soft(&self, input_db: f32) -> f32 {
        let t = self.threshold_db;
        let r = self.ratio;
        let k = self.knee_db;

        if (input_db - t).abs() > k * 0.5 {
            // Outside the soft-knee window — full hard-knee math.
            if input_db <= t {
                0.0
            } else {
                (t + (input_db - t) / r) - input_db
            }
        } else {
            // Inside the soft-knee window — quadratic interpolation.
            let x = input_db - t + k * 0.5;
            (1.0 / r - 1.0) * x * x / (2.0 * k)
        }
    }

    /// Compute gain reduction in dB for a given input level in dB.
    ///
    /// Public entry point that dispatches to the hard or soft path.
    #[inline]
    #[must_use]
    pub fn compute_gain_db(&self, input_db: f32) -> f32 {
        if self.knee_db <= 0.0 {
            self.compute_gain_db_hard(input_db)
        } else {
            self.compute_gain_db_soft(input_db)
        }
    }

    /// Process a single sample.
    ///
    /// Uses [`dsp_util::db_to_amplitude_lut`] (256-entry table, linear
    /// interp) for the gain-stage `dB → linear` step, avoiding `powf` in
    /// the inner loop. The hard-knee path bypasses the soft-knee
    /// conditional entirely.
    #[inline]
    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let env = self.detector.process(input);
        let env_db = dsp_util::amplitude_to_db(env);
        let gain_db = if self.knee_db <= 0.0 {
            self.compute_gain_db_hard(env_db)
        } else {
            self.compute_gain_db_soft(env_db)
        } + self.makeup_db;
        input * dsp_util::db_to_amplitude_lut(gain_db)
    }

    /// Process a buffer in place.
    #[inline]
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for s in buffer.iter_mut() {
            *s = self.process_sample(*s);
        }
    }
}

/// Brick-wall limiter.
///
/// Prevents signal from exceeding the ceiling. Uses fast attack
/// and configurable release for transparent limiting.
///
/// `ceiling_db` and `release` are private because they shadow values inside
/// the underlying [`Compressor`] — modifying them directly would not
/// propagate to the gain stage. Use the typed accessors instead.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Limiter {
    /// Ceiling in dB (typically 0.0 or -0.1).
    ceiling_db: f32,
    /// Release time in seconds.
    release: f32,
    /// Internal compressor with infinity ratio.
    compressor: Compressor,
}

impl Limiter {
    /// Create a new limiter.
    ///
    /// `ceiling_db` is the maximum output level (e.g., -0.1 dB).
    /// `release` is the release time in seconds.
    #[must_use]
    pub fn new(ceiling_db: f32, release: f32, sample_rate: f32) -> Self {
        let mut comp = Compressor::new(ceiling_db, f32::MAX, 0.0001, release, sample_rate);
        comp.knee_db = 0.0;
        Self {
            ceiling_db,
            release,
            compressor: comp,
        }
    }

    /// Returns the current ceiling in dB.
    #[must_use]
    pub fn ceiling_db(&self) -> f32 {
        self.ceiling_db
    }

    /// Set the ceiling in dB. Propagates to the internal gain stage.
    pub fn set_ceiling_db(&mut self, ceiling_db: f32) {
        self.ceiling_db = ceiling_db;
        self.compressor.threshold_db = ceiling_db;
    }

    /// Returns the current release time in seconds.
    #[must_use]
    pub fn release(&self) -> f32 {
        self.release
    }

    /// Process a single sample.
    #[inline]
    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        self.compressor.process_sample(input)
    }

    /// Process a buffer in place.
    #[inline]
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for s in buffer.iter_mut() {
            *s = self.process_sample(*s);
        }
    }
}

/// Noise gate.
///
/// Silences signal below a threshold. Supports configurable
/// attack, hold, and release times.
///
/// `threshold_db` is `pub` because it is read directly each sample with no
/// internal coupling — modifying it takes effect immediately. The smoothing
/// coefficients (attack/hold/release) are private since they are
/// pre-computed at construction from time values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoiseGate {
    /// Threshold in dB. Read directly each sample — modify any time.
    pub threshold_db: f32,
    /// Envelope detector.
    detector: LevelDetector,
    /// Current gate gain (0.0 = closed, 1.0 = open).
    gate_gain: f32,
    /// Hold counter (samples remaining before release).
    hold_counter: u32,
    /// Hold time in samples.
    hold_samples: u32,
    /// Gate opening smoothing coefficient (fast).
    attack_coeff: f32,
    /// Gate closing smoothing coefficient (matches release time).
    release_coeff: f32,
}

impl NoiseGate {
    /// Create a new noise gate.
    ///
    /// * `threshold_db` - Gate threshold in dB
    /// * `attack` - Attack time in seconds
    /// * `hold` - Hold time in seconds
    /// * `release` - Release time in seconds
    #[must_use]
    pub fn new(threshold_db: f32, attack: f32, hold: f32, release: f32, sample_rate: f32) -> Self {
        let attack_time = attack.max(0.001); // minimum 1ms to avoid clicks
        Self {
            threshold_db,
            detector: LevelDetector::new(attack, release, sample_rate),
            gate_gain: 0.0,
            hold_counter: 0,
            hold_samples: (hold * sample_rate) as u32,
            attack_coeff: 1.0 - (-1.0 / (attack_time * sample_rate)).exp(),
            release_coeff: if release > 0.0 {
                1.0 - (-1.0 / (release * sample_rate)).exp()
            } else {
                1.0
            },
        }
    }

    /// Process a single sample.
    #[inline]
    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let env = self.detector.process(input);
        let env_db = dsp_util::amplitude_to_db(env);

        let target = if env_db >= self.threshold_db {
            self.hold_counter = self.hold_samples;
            1.0
        } else if self.hold_counter > 0 {
            self.hold_counter -= 1;
            1.0
        } else {
            0.0
        };

        // Smooth the gate gain: fast attack, slow release
        let coeff = if target > self.gate_gain {
            self.attack_coeff
        } else {
            self.release_coeff
        };
        self.gate_gain += coeff * (target - self.gate_gain);
        self.gate_gain = crate::flush_denormal(self.gate_gain);

        input * self.gate_gain
    }

    /// Process a buffer in place.
    #[inline]
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for s in buffer.iter_mut() {
            *s = self.process_sample(*s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_envelope_detector() {
        let mut det = LevelDetector::new(0.001, 0.01, 44100.0);
        // Feed a loud signal
        for _ in 0..1000 {
            let _ = det.process(1.0);
        }
        assert!(det.current > 0.9, "detector should track input");
        // Release
        for _ in 0..10000 {
            let _ = det.process(0.0);
        }
        assert!(det.current < 0.01, "detector should release");
    }

    #[test]
    fn test_compressor_below_threshold() {
        let mut comp = Compressor::new(-10.0, 4.0, 0.001, 0.01, 44100.0);
        // Very quiet signal should pass through unaffected
        let out = comp.process_sample(0.01);
        assert!(out.is_finite());
    }

    #[test]
    fn test_compressor_reduces_loud() {
        let mut comp = Compressor::new(-20.0, 4.0, 0.0, 0.01, 44100.0);
        // Feed loud signal to build up envelope
        for _ in 0..1000 {
            let _ = comp.process_sample(1.0);
        }
        let out = comp.process_sample(1.0);
        // Output should be reduced
        assert!(
            out < 1.0,
            "compressor should reduce loud signals, got {out}"
        );
    }

    #[test]
    fn test_compressor_soft_knee() {
        let mut comp = Compressor::new(-20.0, 4.0, 0.001, 0.01, 44100.0);
        comp.knee_db = 6.0;
        let gain = comp.compute_gain_db(-17.0); // Within knee
        assert!(gain < 0.0, "soft knee should apply some reduction");
        assert!(gain > -3.0, "soft knee reduction should be gentle");
    }

    #[test]
    fn test_limiter() {
        let mut lim = Limiter::new(-0.1, 0.01, 44100.0);
        // Feed loud signal
        for _ in 0..1000 {
            let _ = lim.process_sample(2.0);
        }
        let out = lim.process_sample(2.0);
        assert!(out < 2.0, "limiter should reduce signal");
    }

    #[test]
    fn test_noise_gate_silences() {
        let mut gate = NoiseGate::new(-40.0, 0.001, 0.01, 0.01, 44100.0);
        // Very quiet signal
        for _ in 0..10000 {
            let _ = gate.process_sample(0.001);
        }
        let out = gate.process_sample(0.001);
        assert!(
            out.abs() < 0.002,
            "gate should attenuate quiet signal, got {out}"
        );
    }

    #[test]
    fn test_noise_gate_passes_loud() {
        let mut gate = NoiseGate::new(-40.0, 0.0, 0.01, 0.01, 44100.0);
        // Loud signal should pass — run enough samples for gate to fully open
        for _ in 0..2000 {
            let _ = gate.process_sample(0.5);
        }
        let out = gate.process_sample(0.5);
        assert!(out > 0.3, "gate should pass loud signal, got {out}");
    }

    #[test]
    fn test_serde_roundtrip_compressor() {
        let comp = Compressor::new(-20.0, 4.0, 0.01, 0.1, 44100.0);
        let json = serde_json::to_string(&comp).unwrap();
        let back: Compressor = serde_json::from_str(&json).unwrap();
        assert!((comp.threshold_db - back.threshold_db).abs() < f32::EPSILON);
    }

    #[test]
    fn test_serde_roundtrip_limiter() {
        let lim = Limiter::new(-0.1, 0.01, 44100.0);
        let json = serde_json::to_string(&lim).unwrap();
        let back: Limiter = serde_json::from_str(&json).unwrap();
        assert!((lim.ceiling_db - back.ceiling_db).abs() < f32::EPSILON);
    }

    #[test]
    fn test_serde_roundtrip_gate() {
        let gate = NoiseGate::new(-40.0, 0.001, 0.01, 0.05, 44100.0);
        let json = serde_json::to_string(&gate).unwrap();
        let back: NoiseGate = serde_json::from_str(&json).unwrap();
        assert!((gate.threshold_db - back.threshold_db).abs() < f32::EPSILON);
    }

    /// O14 — Compressor with `ratio=1.0` is unity gain even above threshold.
    ///
    /// At 1:1 the gain-reduction formula collapses to zero dB, so a loud
    /// signal must pass through unchanged once the envelope has settled.
    /// Verifies the formula has no off-by-one or branch error at the boundary.
    #[test]
    fn test_compressor_ratio_one_is_unity() {
        let mut comp = Compressor::new(-20.0, 1.0, 0.0, 0.01, 44100.0);
        // Settle the envelope detector with a loud above-threshold signal.
        for _ in 0..2000 {
            let _ = comp.process_sample(1.0);
        }
        let out = comp.process_sample(1.0);
        assert!(
            (out - 1.0).abs() < 1e-3,
            "ratio=1.0 must be unity gain, got {out}"
        );
    }

    /// O14 — Hold timer keeps the gate open after the signal drops below threshold.
    ///
    /// After a loud burst opens the gate, switching to a small sub-threshold
    /// signal should still pass through the gate during the hold window, then
    /// close after the timer expires. Distinguishes hold from release.
    #[test]
    fn test_noise_gate_hold_timer_keeps_gate_open() {
        let sr = 44100.0;
        let hold = 0.1; // 100 ms — ample window to observe both phases
        let mut gate = NoiseGate::new(-10.0, 0.0, hold, 0.001, sr);

        // Open the gate with a loud above-threshold signal.
        for _ in 0..1000 {
            let _ = gate.process_sample(1.0);
        }

        // Switch to a small sub-threshold signal; let the envelope detector
        // settle below threshold (1ms release ≪ 500 samples).
        for _ in 0..500 {
            let _ = gate.process_sample(0.05);
        }

        // Still inside the hold window — gate should remain open.
        let mid_hold = gate.process_sample(0.05);
        assert!(
            mid_hold > 0.04,
            "gate must remain open during hold window, got {mid_hold}"
        );

        // Run past hold expiry plus enough samples for the release to take effect.
        let hold_samples = (hold * sr) as usize;
        for _ in 0..hold_samples + 5000 {
            let _ = gate.process_sample(0.05);
        }
        let after_hold = gate.process_sample(0.05);
        assert!(
            after_hold < 0.005,
            "gate must close after hold expires, got {after_hold}"
        );
    }

    /// O14 — Limiter passes input that's exactly at the ceiling unchanged.
    ///
    /// The compressor branch uses `input_db <= threshold`, so a signal sitting
    /// precisely on the ceiling should incur zero gain reduction. Anything
    /// above must be brought back down. Catches a `<` vs `<=` regression at
    /// the boundary.
    #[test]
    fn test_limiter_ceiling_exact_match_passes_through() {
        let ceiling_db = -3.0;
        let amp_at_ceiling = 10f32.powf(ceiling_db / 20.0);
        let mut lim = Limiter::new(ceiling_db, 0.01, 44100.0);

        // Settle envelope at exactly the ceiling.
        for _ in 0..2000 {
            let _ = lim.process_sample(amp_at_ceiling);
        }
        let out_at = lim.process_sample(amp_at_ceiling);
        assert!(
            (out_at - amp_at_ceiling).abs() < 1e-3,
            "input at ceiling must pass through, got {out_at} (expected {amp_at_ceiling})"
        );

        // Now push above ceiling — limiter must reduce it.
        let above = amp_at_ceiling * 2.0;
        for _ in 0..2000 {
            let _ = lim.process_sample(above);
        }
        let out_above = lim.process_sample(above);
        assert!(
            out_above < above,
            "input above ceiling must be reduced, got {out_above} (input {above})"
        );
    }
}
