//! Acoustic analysis metrics from goonj.
//!
//! Wraps goonj's ISO 3382-1 analysis functions to compute room acoustic
//! metrics from an impulse response: clarity (C50, C80), definition (D50),
//! speech transmission index (STI), and reverberation time (RT60).

use serde::{Deserialize, Serialize};

use goonj::analysis::{clarity_c50, clarity_c80, definition_d50, sti_estimate};
use goonj::impulse::ImpulseResponse;
use goonj::room::AcousticRoom;

use crate::error::{NaadError, Result};

use super::room::RoomReverbConfig;

/// Room acoustic metrics computed from an impulse response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomMetrics {
    /// Clarity C50 in dB — early (0-50ms) to late energy ratio.
    pub c50: f32,
    /// Clarity C80 in dB — early (0-80ms) to late energy ratio.
    pub c80: f32,
    /// Definition D50 — fraction of energy in first 50ms (0.0-1.0).
    pub d50: f32,
    /// Speech Transmission Index (0.0-1.0).
    pub sti: f32,
    /// Reverberation time RT60 in seconds.
    pub rt60: f32,
}

/// Analyze an impulse response and compute room acoustic metrics.
///
/// Wraps goonj analysis functions to produce a complete set of metrics
/// from a broadband impulse response.
///
/// # Errors
///
/// Returns [`NaadError::ComputationError`] if the impulse response is empty
/// or the sample rate is zero.
pub fn analyze_impulse_response(ir: &[f32], sample_rate: u32) -> Result<RoomMetrics> {
    if ir.is_empty() {
        return Err(NaadError::ComputationError {
            message: "impulse response is empty".into(),
        });
    }
    if sample_rate == 0 {
        return Err(NaadError::ComputationError {
            message: "sample rate must be > 0".into(),
        });
    }

    let goonj_ir = ImpulseResponse {
        samples: ir.to_vec(),
        sample_rate,
        rt60: 0.0, // will be computed below
    };

    let c50 = clarity_c50(&goonj_ir);
    let c80 = clarity_c80(&goonj_ir);
    let d50 = definition_d50(&goonj_ir);
    let sti = sti_estimate(&goonj_ir);
    let rt60 = estimate_rt60(ir, sample_rate);

    Ok(RoomMetrics {
        c50,
        c80,
        d50,
        sti,
        rt60,
    })
}

/// Quick RT60 estimate from an impulse response using Sabine's equation.
///
/// Estimates absorption from the energy decay of the IR and applies
/// Sabine's formula. For a more accurate result, use the full
/// [`analyze_impulse_response`] function.
#[must_use]
#[inline]
pub fn estimate_rt60(ir: &[f32], sample_rate: u32) -> f32 {
    if ir.is_empty() || sample_rate == 0 {
        return 0.0;
    }

    // Estimate RT60 from energy decay curve
    // Find the time at which energy drops by 60 dB
    let total_energy: f32 = ir.iter().map(|&s| s * s).sum();
    if total_energy < f32::EPSILON {
        return 0.0;
    }

    let mut cumulative = 0.0_f32;
    let threshold = total_energy * 0.001; // -30 dB point (we'll double for RT60)

    for (i, &s) in ir.iter().enumerate() {
        cumulative += s * s;
        if cumulative >= total_energy - threshold {
            // Time to reach -30 dB, double for RT60 estimate
            let t30 = i as f32 / sample_rate as f32;
            return t30 * 2.0;
        }
    }

    // Fallback: use full IR length as upper bound
    ir.len() as f32 / sample_rate as f32
}

/// One wall's contribution to RT60 if its absorption is increased.
///
/// Returned by [`suggest_absorption`] — sort the result by `rt60_sensitivity`
/// (most-negative first) to find the walls that would shorten reverb time
/// the most for a given amount of acoustic treatment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallAbsorptionAdvice {
    /// Index of the wall in the shoebox geometry (0 = floor, 1 = ceiling, 2-5 = side walls).
    pub wall_index: usize,
    /// Change in RT60 (seconds) per unit absorption bump on this wall.
    /// Negative values shorten reverb (the desirable case for taming a live room).
    pub rt60_sensitivity: f32,
    /// Current average absorption of this wall (0.0 = perfectly reflective, 1.0 = perfectly absorptive).
    pub current_absorption: f32,
}

/// Recommend acoustic treatment placement to bring a virtual room toward a target RT60.
///
/// Builds a goonj [`AcousticRoom`] from `config`, then asks goonj which walls
/// would have the most impact on reverb time if their absorption coefficient
/// were increased. Useful for mix-room treatment planning and "what-if"
/// experiments before buying physical absorbers.
///
/// # Errors
///
/// Returns [`NaadError::ComputationError`] if the wall material name is
/// unknown or the room dimensions are invalid.
pub fn suggest_absorption(
    config: &RoomReverbConfig,
    target_rt60: f32,
) -> Result<Vec<WallAbsorptionAdvice>> {
    let material = super::material_by_name(&config.wall_material_name).ok_or_else(|| {
        NaadError::ComputationError {
            message: format!("unknown wall material: {}", config.wall_material_name),
        }
    })?;

    if config.length <= 0.0 || config.width <= 0.0 || config.height <= 0.0 {
        return Err(NaadError::ComputationError {
            message: "room dimensions must be positive".into(),
        });
    }

    let room = AcousticRoom::shoebox(config.length, config.width, config.height, material);
    let suggestions = goonj::analysis::suggest_absorption_placement(&room, target_rt60);

    Ok(suggestions
        .into_iter()
        .map(|s| WallAbsorptionAdvice {
            wall_index: s.wall_index,
            rt60_sensitivity: s.rt60_sensitivity,
            current_absorption: s.current_absorption,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a synthetic exponentially decaying impulse response.
    fn synthetic_ir(sample_rate: u32, duration_secs: f32, decay_rate: f32) -> Vec<f32> {
        let len = (sample_rate as f32 * duration_secs) as usize;
        (0..len)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (-decay_rate * t).exp()
            })
            .collect()
    }

    #[test]
    fn test_analyze_synthetic_ir() {
        let ir = synthetic_ir(48000, 1.0, 5.0);
        let metrics = analyze_impulse_response(&ir, 48000).unwrap();
        assert!(metrics.c50.is_finite(), "C50 should be finite");
        assert!(metrics.c80.is_finite(), "C80 should be finite");
        assert!(metrics.d50.is_finite(), "D50 should be finite");
        assert!(metrics.sti.is_finite(), "STI should be finite");
        assert!(metrics.rt60.is_finite(), "RT60 should be finite");
        assert!(metrics.rt60 > 0.0, "RT60 should be positive");
    }

    #[test]
    fn test_empty_ir_errors() {
        assert!(analyze_impulse_response(&[], 48000).is_err());
    }

    #[test]
    fn test_zero_sample_rate_errors() {
        assert!(analyze_impulse_response(&[1.0, 0.5], 0).is_err());
    }

    #[test]
    fn test_estimate_rt60_basic() {
        let ir = synthetic_ir(48000, 2.0, 3.0);
        let rt60 = estimate_rt60(&ir, 48000);
        assert!(rt60 > 0.0, "RT60 should be positive");
        assert!(rt60.is_finite(), "RT60 should be finite");
    }

    #[test]
    fn test_estimate_rt60_empty() {
        assert_eq!(estimate_rt60(&[], 48000), 0.0);
    }

    #[test]
    fn test_serde_roundtrip() {
        let ir = synthetic_ir(48000, 0.5, 5.0);
        let metrics = analyze_impulse_response(&ir, 48000).unwrap();
        let json = serde_json::to_string(&metrics).unwrap();
        let back: RoomMetrics = serde_json::from_str(&json).unwrap();
        assert!((metrics.c50 - back.c50).abs() < f32::EPSILON);
        assert!((metrics.sti - back.sti).abs() < f32::EPSILON);
    }

    fn live_room() -> RoomReverbConfig {
        RoomReverbConfig {
            length: 8.0,
            width: 6.0,
            height: 4.0,
            wall_material_name: "concrete".to_string(),
            source_position: [4.0, 2.0, 3.0],
            listener_position: [4.0, 2.0, 3.0],
            sample_rate: 48000,
        }
    }

    #[test]
    fn test_suggest_absorption_returns_advice() {
        let advice = suggest_absorption(&live_room(), 0.5).unwrap();
        assert!(!advice.is_empty(), "should return suggestions for shoebox");
        for a in &advice {
            assert!(a.rt60_sensitivity.is_finite());
            assert!(a.current_absorption.is_finite());
        }
    }

    #[test]
    fn test_suggest_absorption_invalid_material() {
        let mut cfg = live_room();
        cfg.wall_material_name = "kryptonite".to_string();
        assert!(suggest_absorption(&cfg, 0.5).is_err());
    }

    #[test]
    fn test_suggest_absorption_invalid_dimensions() {
        let mut cfg = live_room();
        cfg.length = -1.0;
        assert!(suggest_absorption(&cfg, 0.5).is_err());
    }

    #[test]
    fn test_advice_serde_roundtrip() {
        let advice = WallAbsorptionAdvice {
            wall_index: 2,
            rt60_sensitivity: -0.15,
            current_absorption: 0.05,
        };
        let json = serde_json::to_string(&advice).unwrap();
        let back: WallAbsorptionAdvice = serde_json::from_str(&json).unwrap();
        assert_eq!(advice.wall_index, back.wall_index);
        assert!((advice.rt60_sensitivity - back.rt60_sensitivity).abs() < f32::EPSILON);
    }
}
