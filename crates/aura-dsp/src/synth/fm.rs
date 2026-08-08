//! Multi-operator FM (Frequency Modulation) synthesis.
//!
//! Classic FM synthesis with configurable operator count (up to 6) and
//! routing algorithms. Each operator is a sine oscillator with its own
//! ADSR envelope, output level, and optional self-feedback.

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::envelope::Adsr;
use crate::error::Result;

/// Maximum number of FM operators supported.
const MAX_OPERATORS: usize = 6;

/// Routing algorithm for FM operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum FmAlgorithm {
    /// 2-op serial: op1 modulates op2, op2 is carrier.
    Serial2,
    /// 2-op parallel: op1 and op2 both output (additive).
    Parallel2,
    /// 4-op serial chain: op1 -> op2 -> op3 -> op4 (carrier).
    Serial4,
    /// 4-op stack: (op1+op2) modulates (op3+op4), latter pair is carrier.
    Stack4,
    /// Custom algorithm — caller manages modulation externally.
    Custom,
}

/// A single FM operator: sine oscillator + envelope + level + feedback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FmOperator {
    /// Frequency in Hz.
    frequency: f32,
    /// Current oscillator phase (0..1).
    phase: f32,
    /// ADSR envelope for this operator.
    envelope: Adsr,
    /// Output level (0.0 to 1.0).
    output_level: f32,
    /// Self-feedback amount (0.0 to 1.0).
    feedback: f32,
    /// Previous output for feedback path.
    feedback_state: f32,
    /// Sample rate in Hz.
    sample_rate: f32,
}

impl FmOperator {
    /// Create a new FM operator at the given frequency.
    ///
    /// # Errors
    ///
    /// Returns error if sample_rate or envelope parameters are invalid.
    pub fn new(frequency: f32, sample_rate: f32) -> Result<Self> {
        let envelope = Adsr::with_sample_rate(0.01, 0.1, 0.8, 0.3, sample_rate)?;
        Ok(Self {
            frequency,
            phase: 0.0,
            envelope,
            output_level: 1.0,
            feedback: 0.0,
            feedback_state: 0.0,
            sample_rate,
        })
    }

    /// Set the operator frequency.
    pub fn set_frequency(&mut self, freq: f32) {
        self.frequency = freq;
    }

    /// Set the output level (clamped to 0.0..1.0).
    pub fn set_level(&mut self, level: f32) {
        self.output_level = level.clamp(0.0, 1.0);
    }

    /// Set the self-feedback amount (clamped to 0.0..1.0).
    pub fn set_feedback(&mut self, amount: f32) {
        self.feedback = amount.clamp(0.0, 1.0);
    }

    /// Trigger the operator envelope.
    pub fn gate_on(&mut self) {
        self.envelope.gate_on();
    }

    /// Release the operator envelope.
    pub fn gate_off(&mut self) {
        self.envelope.gate_off();
    }

    /// Generate the next sample with external phase modulation input.
    ///
    /// `phase_mod` is added to the operator's phase before computing sine.
    /// Use this directly for custom FM topologies beyond the built-in algorithms.
    #[inline]
    #[must_use]
    pub fn next_sample(&mut self, phase_mod: f32) -> f32 {
        let fb = self.feedback * self.feedback_state;
        let mod_phase = self.phase + phase_mod + fb;
        let out = (mod_phase * std::f32::consts::TAU).sin();
        let env = self.envelope.next_value();
        let result = out * env * self.output_level;

        self.feedback_state = crate::flush_denormal(result);

        // Advance phase
        let phase_inc = self.frequency / self.sample_rate;
        self.phase += phase_inc;
        self.phase -= self.phase.floor();

        result
    }

    /// Returns a mutable reference to the operator's envelope.
    pub fn envelope_mut(&mut self) -> &mut Adsr {
        &mut self.envelope
    }

    /// Returns a reference to the operator's envelope.
    #[must_use]
    pub fn envelope(&self) -> &Adsr {
        &self.envelope
    }
}

/// Multi-operator FM synthesis engine.
///
/// Supports up to 6 operators routed through configurable algorithms.
/// Operators live in a [`SmallVec`] sized for the `MAX_OPERATORS = 6`
/// cap, so allocation stays on the stack for every supported configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FmSynthEngine {
    /// Operators (up to 6, always stack-resident).
    operators: SmallVec<[FmOperator; MAX_OPERATORS]>,
    /// Routing algorithm.
    algorithm: FmAlgorithm,
    /// Sample rate in Hz.
    sample_rate: f32,
}

impl FmSynthEngine {
    /// Create a new FM synth engine with the given number of operators.
    ///
    /// Operators are initialised as 440 Hz sine oscillators.
    ///
    /// # Errors
    ///
    /// Returns error if `num_operators` is 0 or > 6, or sample_rate is invalid.
    pub fn new(num_operators: usize, sample_rate: f32) -> Result<Self> {
        if num_operators == 0 || num_operators > MAX_OPERATORS {
            return Err(crate::error::NaadError::InvalidParameter {
                name: "num_operators".to_string(),
                reason: format!("must be 1..={MAX_OPERATORS}"),
            });
        }
        if sample_rate <= 0.0 || !sample_rate.is_finite() {
            return Err(crate::error::NaadError::InvalidSampleRate { sample_rate });
        }

        let mut operators: SmallVec<[FmOperator; MAX_OPERATORS]> = SmallVec::new();
        for _ in 0..num_operators {
            operators.push(FmOperator::new(440.0, sample_rate)?);
        }

        Ok(Self {
            operators,
            algorithm: FmAlgorithm::Serial2,
            sample_rate,
        })
    }

    /// Set the routing algorithm.
    pub fn set_algorithm(&mut self, algorithm: FmAlgorithm) {
        self.algorithm = algorithm;
    }

    /// Set the frequency of an operator by index.
    ///
    /// Returns `None` if `index` is out of range, allowing callers to detect
    /// — and not silently ignore — bad indices. The previous fire-and-forget
    /// signature was a discoverability hazard for callers building algorithms
    /// dynamically.
    pub fn set_operator_freq(&mut self, index: usize, freq: f32) -> Option<()> {
        let op = self.operators.get_mut(index)?;
        op.set_frequency(freq);
        Some(())
    }

    /// Set the output level of an operator by index.
    ///
    /// Returns `None` if `index` is out of range. See [`Self::set_operator_freq`]
    /// for rationale.
    pub fn set_operator_level(&mut self, index: usize, level: f32) -> Option<()> {
        let op = self.operators.get_mut(index)?;
        op.set_level(level);
        Some(())
    }

    /// Trigger all operator envelopes (note on).
    pub fn note_on(&mut self) {
        for op in &mut self.operators {
            op.gate_on();
        }
    }

    /// Release all operator envelopes (note off).
    pub fn note_off(&mut self) {
        for op in &mut self.operators {
            op.gate_off();
        }
    }

    /// Returns a mutable reference to an operator by index.
    pub fn operator_mut(&mut self, index: usize) -> Option<&mut FmOperator> {
        self.operators.get_mut(index)
    }

    /// Returns a reference to an operator by index.
    #[must_use]
    pub fn operator(&self, index: usize) -> Option<&FmOperator> {
        self.operators.get(index)
    }

    /// Generate the next output sample by routing operators through
    /// the current algorithm.
    #[inline]
    #[must_use]
    pub fn next_sample(&mut self) -> f32 {
        let n = self.operators.len();
        match self.algorithm {
            FmAlgorithm::Serial2 => self.process_serial2(n),
            FmAlgorithm::Parallel2 => self.process_parallel2(n),
            FmAlgorithm::Serial4 => self.process_serial4(n),
            FmAlgorithm::Stack4 => self.process_stack4(n),
            FmAlgorithm::Custom => {
                // Custom: all operators output in parallel (no modulation).
                let mut sum = 0.0f32;
                for op in &mut self.operators {
                    sum += op.next_sample(0.0);
                }
                sum / n.max(1) as f32
            }
        }
    }

    /// Fill a buffer with samples.
    #[inline]
    pub fn fill_buffer(&mut self, buffer: &mut [f32]) {
        for s in buffer.iter_mut() {
            *s = self.next_sample();
        }
    }

    /// Serial2: op[0] -> op[1] -> out (uses first 2 operators).
    #[inline]
    fn process_serial2(&mut self, n: usize) -> f32 {
        if n < 2 {
            return self.operators[0].next_sample(0.0);
        }
        // Safety: we checked n >= 2; use index splitting to avoid borrow issues.
        let (first, rest) = self.operators.split_at_mut(1);
        let mod_out = first[0].next_sample(0.0);
        rest[0].next_sample(mod_out)
    }

    /// Parallel2: (op[0] + op[1]) / 2 -> out.
    #[inline]
    fn process_parallel2(&mut self, n: usize) -> f32 {
        if n < 2 {
            return self.operators[0].next_sample(0.0);
        }
        let a = self.operators[0].next_sample(0.0);
        let b = self.operators[1].next_sample(0.0);
        (a + b) * 0.5
    }

    /// Serial4: op[0] -> op[1] -> op[2] -> op[3] -> out.
    #[inline]
    fn process_serial4(&mut self, n: usize) -> f32 {
        let mut mod_signal = 0.0f32;
        let count = n.min(4);
        for i in 0..count {
            mod_signal = self.operators[i].next_sample(mod_signal);
        }
        mod_signal
    }

    /// Stack4: (op[0]+op[1]) modulates (op[2]+op[3]).
    #[inline]
    fn process_stack4(&mut self, n: usize) -> f32 {
        if n < 4 {
            return self.process_serial4(n);
        }
        let (first_pair, second_pair) = self.operators.split_at_mut(2);
        let mod_a = first_pair[0].next_sample(0.0);
        let mod_b = first_pair[1].next_sample(0.0);
        let mod_sum = (mod_a + mod_b) * 0.5;
        let car_a = second_pair[0].next_sample(mod_sum);
        let car_b = second_pair[1].next_sample(mod_sum);
        (car_a + car_b) * 0.5
    }

    /// Check if any operator envelope is still active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.operators.iter().any(|op| op.envelope.is_active())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_output() {
        let mut engine = FmSynthEngine::new(2, 44100.0).unwrap();
        engine.set_operator_freq(0, 440.0);
        engine.set_operator_freq(1, 440.0);
        engine.set_algorithm(FmAlgorithm::Serial2);
        engine.note_on();

        let mut buf = [0.0f32; 1024];
        engine.fill_buffer(&mut buf);
        assert!(buf.iter().any(|&s| s.abs() > 0.01), "should produce output");
        assert!(buf.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn test_serial_vs_parallel_differ() {
        let mut serial = FmSynthEngine::new(2, 44100.0).unwrap();
        serial.set_operator_freq(0, 200.0);
        serial.set_operator_freq(1, 440.0);
        serial.set_operator_level(0, 1.0);
        serial.set_algorithm(FmAlgorithm::Serial2);
        serial.note_on();

        let mut parallel = FmSynthEngine::new(2, 44100.0).unwrap();
        parallel.set_operator_freq(0, 200.0);
        parallel.set_operator_freq(1, 440.0);
        parallel.set_operator_level(0, 1.0);
        parallel.set_algorithm(FmAlgorithm::Parallel2);
        parallel.note_on();

        let mut buf_s = [0.0f32; 512];
        let mut buf_p = [0.0f32; 512];
        serial.fill_buffer(&mut buf_s);
        parallel.fill_buffer(&mut buf_p);

        // The outputs should differ due to different routing.
        let diff: f32 = buf_s
            .iter()
            .zip(buf_p.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(
            diff > 0.1,
            "serial and parallel should produce different spectra"
        );
    }

    #[test]
    fn test_serde_roundtrip() {
        let engine = FmSynthEngine::new(4, 44100.0).unwrap();
        let json = serde_json::to_string(&engine).unwrap();
        let back: FmSynthEngine = serde_json::from_str(&json).unwrap();
        assert_eq!(engine.operators.len(), back.operators.len());
        assert_eq!(engine.algorithm, back.algorithm);
    }

    #[test]
    fn test_four_op_algorithms() {
        let mut engine = FmSynthEngine::new(4, 44100.0).unwrap();
        engine.set_algorithm(FmAlgorithm::Serial4);
        engine.note_on();
        let mut buf = [0.0f32; 256];
        engine.fill_buffer(&mut buf);
        assert!(buf.iter().any(|&s| s.abs() > 0.001));

        let mut engine2 = FmSynthEngine::new(4, 44100.0).unwrap();
        engine2.set_algorithm(FmAlgorithm::Stack4);
        engine2.note_on();
        let mut buf2 = [0.0f32; 256];
        engine2.fill_buffer(&mut buf2);
        assert!(buf2.iter().any(|&s| s.abs() > 0.001));
    }
}
