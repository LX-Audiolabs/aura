//! Coupled room decay analysis.
//!
//! Wraps goonj's coupled-rooms simulation: two acoustic spaces joined by a
//! portal (e.g., live room ↔ control room, stage ↔ audience). The combined
//! decay is *not* a single exponential — energy in the smaller, more
//! absorptive room decays first ("early"), then is sustained by spillover
//! from the larger / more reverberant room ("late"). This produces the
//! characteristic *double-slope* decay heard in coupled-volume halls and
//! cathedrals with reverberant side chapels.

use serde::{Deserialize, Serialize};

use goonj::coupled::{CoupledRooms, coupled_room_decay};
use goonj::portal::Portal;
use goonj::room::AcousticRoom;
use hisab::Vec3;

use crate::error::{NaadError, Result};

use super::room::RoomReverbConfig;

/// A portal opening connecting two rooms.
///
/// `position` is the centre of the opening; `normal` points from `room_a`
/// into `room_b`. Width/height are in metres.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoupledPortal {
    /// Centre position `[x, y, z]` in metres (in `room_a`'s coordinate frame).
    pub position: [f32; 3],
    /// Normal direction `[x, y, z]` (unit-ish), pointing from `room_a` into `room_b`.
    pub normal: [f32; 3],
    /// Opening width in metres.
    pub width: f32,
    /// Opening height in metres.
    pub height: f32,
}

/// Configuration for a two-room coupled-decay simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoupledRoomConfig {
    /// The first (typically smaller / more absorptive) room.
    pub room_a: RoomReverbConfig,
    /// The second (typically larger / more reverberant) room.
    pub room_b: RoomReverbConfig,
    /// Portal connecting the two rooms.
    pub portal: CoupledPortal,
}

/// Result of a coupled-decay analysis.
///
/// Two RT60 values describe the double-slope: `rt60_early` is the initial
/// fast decay (energy escaping through the portal + absorbed by the first
/// room), `rt60_late` is the slower tail sustained by spillover. The
/// `coupling_strength` field summarises how strongly the rooms exchange
/// energy via the portal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoupledDecayResult {
    /// RT60 of the early (fast) decay component in seconds.
    pub rt60_early: f32,
    /// RT60 of the late (slow) decay component in seconds.
    pub rt60_late: f32,
    /// Relative amplitude of the early component (0.0–1.0).
    pub early_amplitude: f32,
    /// Coupling strength (0.0 = isolated rooms, 1.0 = fully coupled).
    pub coupling_strength: f32,
}

/// Run the coupled-decay analysis for a two-room configuration.
///
/// Builds the underlying goonj rooms + portal, then asks goonj for the
/// double-slope decay parameters. Useful for designing live-room /
/// control-room pairs or modelling cathedrals with reverberant side
/// chapels.
///
/// # Errors
///
/// Returns [`NaadError::ComputationError`] if either room's material name
/// is unknown or any room dimension is non-positive.
pub fn analyze_coupled_decay(config: &CoupledRoomConfig) -> Result<CoupledDecayResult> {
    let room_a = build_room(&config.room_a)?;
    let room_b = build_room(&config.room_b)?;

    let portal = Portal {
        position: Vec3::new(
            config.portal.position[0],
            config.portal.position[1],
            config.portal.position[2],
        ),
        normal: Vec3::new(
            config.portal.normal[0],
            config.portal.normal[1],
            config.portal.normal[2],
        ),
        width: config.portal.width,
        height: config.portal.height,
    };

    let coupled = CoupledRooms {
        room_a,
        room_b,
        portal,
    };

    let decay = coupled_room_decay(&coupled);

    Ok(CoupledDecayResult {
        rt60_early: decay.rt60_early,
        rt60_late: decay.rt60_late,
        early_amplitude: decay.early_amplitude,
        coupling_strength: decay.coupling_strength,
    })
}

/// Build a goonj `AcousticRoom` from a naad `RoomReverbConfig`.
fn build_room(config: &RoomReverbConfig) -> Result<AcousticRoom> {
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

    Ok(AcousticRoom::shoebox(
        config.length,
        config.width,
        config.height,
        material,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small_room() -> RoomReverbConfig {
        RoomReverbConfig {
            length: 5.0,
            width: 4.0,
            height: 3.0,
            wall_material_name: "carpet".to_string(),
            source_position: [1.0, 1.5, 1.0],
            listener_position: [4.0, 1.5, 3.0],
            sample_rate: 48000,
        }
    }

    fn large_room() -> RoomReverbConfig {
        RoomReverbConfig {
            length: 20.0,
            width: 15.0,
            height: 8.0,
            wall_material_name: "concrete".to_string(),
            source_position: [10.0, 4.0, 7.0],
            listener_position: [5.0, 4.0, 7.0],
            sample_rate: 48000,
        }
    }

    fn portal() -> CoupledPortal {
        CoupledPortal {
            position: [5.0, 1.5, 2.0],
            normal: [1.0, 0.0, 0.0],
            width: 1.0,
            height: 2.0,
        }
    }

    #[test]
    fn test_coupled_decay_two_rooms() {
        let cfg = CoupledRoomConfig {
            room_a: small_room(),
            room_b: large_room(),
            portal: portal(),
        };
        let decay = analyze_coupled_decay(&cfg).unwrap();
        assert!(decay.rt60_early.is_finite() && decay.rt60_early > 0.0);
        assert!(decay.rt60_late.is_finite() && decay.rt60_late > 0.0);
        assert!((0.0..=1.0).contains(&decay.early_amplitude));
        assert!((0.0..=1.0).contains(&decay.coupling_strength));
    }

    #[test]
    fn test_coupled_decay_late_is_longer_for_reverberant_room_b() {
        // Carpeted room A coupled to concrete room B — late decay should be
        // longer than early decay (energy sustained by the live room).
        let cfg = CoupledRoomConfig {
            room_a: small_room(),
            room_b: large_room(),
            portal: portal(),
        };
        let decay = analyze_coupled_decay(&cfg).unwrap();
        assert!(
            decay.rt60_late >= decay.rt60_early,
            "concrete coupled room should sustain late decay: early={}, late={}",
            decay.rt60_early,
            decay.rt60_late
        );
    }

    #[test]
    fn test_invalid_material_errors() {
        let mut a = small_room();
        a.wall_material_name = "kryptonite".to_string();
        let cfg = CoupledRoomConfig {
            room_a: a,
            room_b: large_room(),
            portal: portal(),
        };
        assert!(analyze_coupled_decay(&cfg).is_err());
    }

    #[test]
    fn test_serde_roundtrip() {
        let cfg = CoupledRoomConfig {
            room_a: small_room(),
            room_b: large_room(),
            portal: portal(),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: CoupledRoomConfig = serde_json::from_str(&json).unwrap();
        assert!((back.portal.width - 1.0).abs() < f32::EPSILON);
    }
}
