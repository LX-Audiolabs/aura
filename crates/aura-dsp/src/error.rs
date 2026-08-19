//! Error types for the aura-dsp synthesis crate.

use serde::{Deserialize, Serialize};
use tracing::warn;

/// Errors that can occur during audio synthesis operations.
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[non_exhaustive]
pub enum DspError {
    /// Frequency is out of valid range (must be > 0 and < Nyquist).
    #[error("invalid frequency {frequency}: must be > 0 and < {nyquist} (Nyquist)")]
    InvalidFrequency {
        /// The invalid frequency value.
        frequency: f32,
        /// The Nyquist frequency (`sample_rate` / 2).
        nyquist: f32,
    },

    /// Sample rate is invalid (must be > 0).
    #[error("invalid sample rate {sample_rate}: must be > 0")]
    InvalidSampleRate {
        /// The invalid sample rate value.
        sample_rate: f32,
    },

    /// A parameter is out of its valid range.
    #[error("invalid parameter '{name}': {reason}")]
    InvalidParameter {
        /// The name of the parameter.
        name: String,
        /// Why the parameter is invalid.
        reason: String,
    },

    /// Buffer operation exceeded capacity.
    #[error("buffer overflow: attempted {attempted} but capacity is {capacity}")]
    BufferOverflow {
        /// The attempted size.
        attempted: usize,
        /// The available capacity.
        capacity: usize,
    },

    /// A computation produced an invalid result.
    #[error("computation error: {message}")]
    ComputationError {
        /// Description of the error.
        message: String,
    },
}

/// Result type alias for aura-dsp operations.
pub type Result<T> = std::result::Result<T, DspError>;

/// Validate that a frequency is within the valid range for a given sample rate.
#[must_use]
pub(crate) fn validate_frequency(frequency: f32, sample_rate: f32) -> Option<DspError> {
    let nyquist = sample_rate / 2.0;
    if frequency <= 0.0 || frequency >= nyquist || !frequency.is_finite() {
        warn!(frequency, nyquist, "invalid frequency");
        Some(DspError::InvalidFrequency { frequency, nyquist })
    } else {
        None
    }
}

/// Validate that a sample rate is positive and finite.
#[must_use]
pub(crate) fn validate_sample_rate(sample_rate: f32) -> Option<DspError> {
    if sample_rate <= 0.0 || !sample_rate.is_finite() {
        warn!(sample_rate, "invalid sample rate");
        Some(DspError::InvalidSampleRate { sample_rate })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = DspError::InvalidFrequency {
            frequency: -1.0,
            nyquist: 22050.0,
        };
        assert!(err.to_string().contains("-1"));
    }

    #[test]
    fn test_error_display_all_variants() {
        let cases: Vec<(DspError, &str)> = vec![
            (
                DspError::InvalidFrequency {
                    frequency: -1.0,
                    nyquist: 22050.0,
                },
                "invalid frequency",
            ),
            (
                DspError::InvalidSampleRate { sample_rate: 0.0 },
                "invalid sample rate",
            ),
            (
                DspError::InvalidParameter {
                    name: "q".into(),
                    reason: "must be > 0".into(),
                },
                "invalid parameter",
            ),
            (
                DspError::BufferOverflow {
                    attempted: 2048,
                    capacity: 1024,
                },
                "buffer overflow",
            ),
            (
                DspError::ComputationError {
                    message: "division by zero".into(),
                },
                "computation error",
            ),
        ];

        for (err, expected_prefix) in &cases {
            let msg = err.to_string();
            assert!(
                msg.contains(expected_prefix),
                "error '{msg}' should contain '{expected_prefix}'"
            );
        }
    }

    #[test]
    fn test_serde_roundtrip() {
        let err = DspError::InvalidParameter {
            name: "q".to_string(),
            reason: "must be > 0".to_string(),
        };
        let json = serde_json::to_string(&err).unwrap();
        let back: DspError = serde_json::from_str(&json).unwrap();
        assert_eq!(err.to_string(), back.to_string());
    }

    #[test]
    fn test_serde_roundtrip_all_variants() {
        let errors = vec![
            DspError::InvalidFrequency {
                frequency: 50000.0,
                nyquist: 22050.0,
            },
            DspError::InvalidSampleRate { sample_rate: -1.0 },
            DspError::BufferOverflow {
                attempted: 100,
                capacity: 50,
            },
            DspError::ComputationError {
                message: "test".into(),
            },
        ];

        for err in &errors {
            let json = serde_json::to_string(err).unwrap();
            let back: DspError = serde_json::from_str(&json).unwrap();
            assert_eq!(err.to_string(), back.to_string());
        }
    }

    #[test]
    fn test_validate_frequency() {
        // Valid
        assert!(validate_frequency(440.0, 44100.0).is_none());
        // Too low
        assert!(validate_frequency(0.0, 44100.0).is_some());
        assert!(validate_frequency(-1.0, 44100.0).is_some());
        // Above Nyquist
        assert!(validate_frequency(23000.0, 44100.0).is_some());
        // NaN
        assert!(validate_frequency(f32::NAN, 44100.0).is_some());
    }

    #[test]
    fn test_validate_sample_rate() {
        assert!(validate_sample_rate(44100.0).is_none());
        assert!(validate_sample_rate(0.0).is_some());
        assert!(validate_sample_rate(-1.0).is_some());
        assert!(validate_sample_rate(f32::NAN).is_some());
        assert!(validate_sample_rate(f32::INFINITY).is_some());
    }
}
