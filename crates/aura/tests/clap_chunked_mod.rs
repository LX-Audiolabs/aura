//! Sample-accurate automation + mono modulation (framework unit level).

use aura::prelude::*;
use aura_core::{TimedParamEvent, apply_at_time, apply_non_chunked, split_points};
use aura_params::ParamFlags;

#[derive(Params)]
struct GainParams {
    #[param(id = 1, name = "Gain", range = "linear(0, 1)", default = 1.0)]
    gain: FloatParam,
    /// Expensive retarget — must not force splits.
    #[param(
        id = 2,
        name = "Mode",
        range = "linear(0, 1)",
        default = 0.0,
        chunk = false
    )]
    mode: FloatParam,
    #[param(
        id = 3,
        name = "Drive",
        range = "linear(0, 1)",
        default = 0.5,
        flags = "automatable | modulatable"
    )]
    drive: FloatParam,
}

#[test]
fn chunked_flag_default_and_opt_out() {
    let p = GainParams::default();
    let infos = p.param_infos();
    assert!(infos[0].flags.contains(ParamFlags::CHUNKED));
    assert!(!infos[1].flags.contains(ParamFlags::CHUNKED));
    assert!(infos[2].flags.contains(ParamFlags::MODULATABLE));
}

#[test]
fn split_points_respect_chunk_flag() {
    let p = GainParams::default();
    let infos = p.param_infos();
    let events = [
        TimedParamEvent::Value {
            sample_offset: 32,
            id: 1,
            plain: 0.25,
        },
        TimedParamEvent::Value {
            sample_offset: 48,
            id: 2, // chunk = false
            plain: 1.0,
        },
        TimedParamEvent::Mod {
            sample_offset: 64,
            id: 3,
            amount: 0.1,
        },
    ];
    // Gain (chunked) + Drive (chunked default) split; Mode does not.
    assert_eq!(split_points(128, &events, &infos), vec![0, 32, 64, 128]);
}

#[test]
fn mono_mod_is_non_destructive() {
    let p = GainParams::default();
    p.drive.set_value(0.5);
    assert!((p.drive.raw_target() - 0.5).abs() < 1e-12);
    assert!((p.drive.effective_target() - 0.5).abs() < 1e-12);

    p.set_mod(3, 0.25);
    assert!((p.drive.raw_target() - 0.5).abs() < 1e-12); // base unchanged
    assert!((p.drive.effective_target() - 0.75).abs() < 1e-12);

    p.set_mod(3, 0.0);
    assert!((p.drive.effective_target() - 0.5).abs() < 1e-12);
}

#[test]
fn apply_events_sample_accurate_order() {
    let p = GainParams::default();
    let infos = p.param_infos();
    let events = [
        TimedParamEvent::Value {
            sample_offset: 0,
            id: 1,
            plain: 0.0,
        },
        TimedParamEvent::Value {
            sample_offset: 50,
            id: 1,
            plain: 1.0,
        },
        TimedParamEvent::Value {
            sample_offset: 10,
            id: 2,
            plain: 0.8,
        },
    ];
    apply_non_chunked(&p, &events, &infos);
    // Mode is non-chunked → applied at block start.
    assert!((p.mode.raw_target() - 0.8).abs() < 1e-12);
    // Gain still default until chunk apply.
    assert!((p.gain.raw_target() - 1.0).abs() < 1e-12);

    apply_at_time(&p, &events, 0, &infos);
    assert!((p.gain.raw_target() - 0.0).abs() < 1e-12);

    apply_at_time(&p, &events, 50, &infos);
    assert!((p.gain.raw_target() - 1.0).abs() < 1e-12);
}
