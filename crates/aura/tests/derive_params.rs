//! Integration coverage for `#[derive(Params)]` / `#[derive(ParamEnum)]`
//! from `aura-derive`: mixed param kinds, nested structs, persist
//! round-trip, meter IDs, single-dispatch setters, and the
//! construction-time collision panic.
//!
//! Exact float equality is asserted where the range math is exact by
//! construction (denormalize(1.0) == max on a linear range, bool 0/1,
//! enum indices); approximations carry explicit tolerances.
#![allow(clippy::float_cmp)]

use std::sync::RwLock;

use aura::params::{MidiSource, ParamFlags};
use aura::prelude::*;

#[derive(Debug, ParamEnum)]
enum Mode {
    Clean,
    #[name = "Crunch+"]
    Crunch,
    Dirt,
}

#[derive(Params)]
struct SubParams {
    #[param(id = 10, name = "Tone", range = "linear(0, 10)", default = 5.0)]
    tone: FloatParam,
}

#[derive(Params)]
struct TestParams {
    #[param(
        id = 1,
        name = "Gain",
        range = "linear(-24, 24)",
        default = 0.0,
        unit = "db",
        smooth = "exp(20)"
    )]
    gain: FloatParam,
    #[param(
        id = 2,
        name = "Octave",
        range = "discrete(-2, 2)",
        default = 0,
        unit = "st"
    )]
    octave: IntParam,
    #[param(id = 3, name = "Bypass", default = 0)]
    bypass: BoolParam,
    #[param(id = 4, name = "Mode", default = 1)]
    mode: EnumParam<Mode>,
    #[param(
        id = 5,
        name = "Cutoff",
        range = "log(20, 20000)",
        default = 1000.0,
        unit = "Hz",
        chunk = false,
        midi_cc = 74
    )]
    cutoff: FloatParam,
    #[meter]
    level: MeterSlot,
    #[nested]
    sub: SubParams,
    #[persist]
    ui_scale: RwLock<f64>,
    #[persist = "tab"]
    active_tab: RwLock<u32>,
}

#[test]
fn infos_cover_all_kinds_in_declaration_order() {
    let p = TestParams::new();
    let infos = p.param_infos();
    let ids: Vec<u32> = infos.iter().map(|i| i.id).collect();
    assert_eq!(ids, vec![1, 2, 3, 4, 5, 10], "own params, then nested");
    assert_eq!(p.count(), 6);
    assert_eq!(infos[0].name, "Gain");
    assert_eq!(infos[0].unit, ParamUnit::Db);
    assert_eq!(infos[0].kind, ParamValueKind::Float);
    assert_eq!(infos[1].kind, ParamValueKind::Int);
    assert_eq!(infos[2].kind, ParamValueKind::Bool);
    assert_eq!(infos[3].kind, ParamValueKind::Enum);
}

#[test]
fn flags_and_midi_baked_from_attributes() {
    let p = TestParams::new();
    let infos = p.param_infos();
    // CHUNKED defaults on; `chunk = false` clears it.
    assert!(
        infos[0]
            .flags
            .contains(ParamFlags::AUTOMATABLE | ParamFlags::CHUNKED)
    );
    assert!(infos[4].flags.contains(ParamFlags::AUTOMATABLE));
    assert!(!infos[4].flags.contains(ParamFlags::CHUNKED));
    assert_eq!(infos[4].midi_map, Some(MidiSource::Cc(74)));
    assert_eq!(infos[4].midi_channel, None);
}

#[test]
fn static_infos_match_instance_infos() {
    let p = TestParams::new();
    let statik = TestParams::param_infos_static();
    let live = p.param_infos();
    assert_eq!(statik.len(), live.len());
    for (s, l) in statik.iter().zip(&live) {
        assert_eq!(s.id, l.id);
        assert_eq!(s.name, l.name);
        assert_eq!(s.default_plain, l.default_plain);
    }
}

#[test]
fn defaults_land_in_params() {
    let p = TestParams::new();
    assert_eq!(p.gain.raw_target(), 0.0);
    assert_eq!(p.octave.value(), 0);
    assert!(!p.bypass.value());
    assert_eq!(p.mode.index(), 1);
    assert_eq!(p.cutoff.raw_target(), 1000.0);
    assert_eq!(p.sub.tone.raw_target(), 5.0);
}

#[test]
fn plain_and_normalized_round_trip() {
    let p = TestParams::new();
    p.set_normalized(1, 1.0);
    assert_eq!(p.get_plain(1), Some(24.0), "linear max at normalized 1.0");
    assert_eq!(p.get_normalized(1), Some(1.0));

    p.set_plain(2, -1.0);
    assert_eq!(p.octave.value(), -1);
    assert_eq!(p.get_normalized(2), Some(0.25), "-1 of [-2, 2]");

    p.set_normalized(3, 0.9);
    assert!(p.bypass.value());
    assert_eq!(p.get_plain(3), Some(1.0));

    // Enum: normalized 1.0 selects the last variant.
    p.set_normalized(4, 1.0);
    assert_eq!(p.mode.index(), 2);

    // Unknown ID: None / no-op.
    assert_eq!(p.get_plain(999), None);
    p.set_plain(999, 1.0);
}

#[test]
fn single_dispatch_setters_return_post_clamp_values() {
    let p = TestParams::new();
    // Discrete range quantizes: normalized 0.9 on [-2, 2] denormalizes
    // to 1.6, which the Int commit rounds to 2.
    let plain = p.set_normalized_returning_plain(2, 0.9);
    assert_eq!(plain, 2.0);
    assert_eq!(p.octave.value(), 2);
    let norm = p.set_normalized_returning_normalized(2, 0.9);
    assert_eq!(norm, 1.0, "readback reflects the rounded commit");
    // Nested dispatch reaches the child in one call.
    let plain = p.set_normalized_returning_plain(10, 1.0);
    assert_eq!(plain, 10.0);
    assert_eq!(p.sub.tone.raw_target(), 10.0);
}

#[test]
fn format_and_parse_defaults() {
    let p = TestParams::new();
    assert_eq!(p.format_value(1, 6.0).as_deref(), Some("6.0 dB"));
    assert_eq!(p.format_value(2, -1.0).as_deref(), Some("-1 st"));
    assert_eq!(p.format_value(3, 1.0).as_deref(), Some("On"));
    assert_eq!(p.format_value(3, 0.0).as_deref(), Some("Off"));
    assert_eq!(p.format_value(4, 1.0).as_deref(), Some("Crunch+"));
    assert_eq!(p.format_value(5, 1500.0).as_deref(), Some("1.5 kHz"));

    // Unit suffix trimmed, exact and lowercased (matches the
    // hand-written smoke-gain behavior).
    assert_eq!(p.parse_value(1, "-12 dB"), Some(-12.0));
    assert_eq!(p.parse_value(1, "-12db"), Some(-12.0));
    assert_eq!(p.parse_value(1, "garbage"), None);
    assert_eq!(p.parse_value(2, "2"), Some(2.0));
    assert_eq!(p.parse_value(3, "on"), Some(1.0));
    assert_eq!(p.parse_value(3, "False"), Some(0.0));
    assert_eq!(
        p.parse_value(4, "dirt"),
        Some(2.0),
        "case-insensitive variant match"
    );
    assert_eq!(p.parse_value(4, "crunch+"), Some(1.0));
    assert_eq!(p.parse_value(4, "nope"), None);
    // Nested param parses through the parent.
    assert_eq!(p.parse_value(10, "7"), Some(7.0));
}

#[test]
fn meter_ids_auto_assign_from_base() {
    let p = TestParams::new();
    assert_eq!(p.meter_ids(), vec![aura::params::METER_ID_BASE]);
    assert_eq!(p.level.id(), aura::params::METER_ID_BASE);
}

#[test]
fn collect_restore_round_trip_including_nested() {
    let p = TestParams::new();
    p.set_plain(1, 6.0);
    p.set_plain(10, 8.0);
    let (ids, values) = p.collect_values();
    assert_eq!(ids, vec![1, 2, 3, 4, 5, 10]);

    let q = TestParams::new();
    q.restore_values(&ids.iter().copied().zip(values).collect::<Vec<_>>());
    assert_eq!(q.get_plain(1), Some(6.0));
    assert_eq!(q.get_plain(10), Some(8.0));
}

#[test]
fn persist_round_trip_and_tolerance() {
    let p = TestParams::new();
    *p.ui_scale.write().expect("lock") = 1.5;
    *p.active_tab.write().expect("lock") = 2;
    let blob = p.serialize_persist();

    let q = TestParams::new();
    q.load_persist(&blob);
    assert_eq!(*q.ui_scale.read().expect("lock"), 1.5);
    assert_eq!(*q.active_tab.read().expect("lock"), 2);

    // Truncated / empty blobs are skipped, leaving current values.
    let r = TestParams::new();
    *r.active_tab.write().expect("lock") = 7;
    r.load_persist(&[]);
    // Count + a partial key-length word: every entry read bails out.
    r.load_persist(&blob[..6]);
    assert_eq!(
        *r.active_tab.read().expect("lock"),
        7,
        "malformed blobs never clobber state"
    );

    // Unknown keys are skipped: a blob advertising entries we don't
    // know just gets ignored for those slots.
    let mut foreign = 1u32.to_le_bytes().to_vec();
    let key = b"future_field";
    foreign.extend_from_slice(&12u32.to_le_bytes());
    foreign.extend_from_slice(key);
    foreign.extend_from_slice(&4u32.to_le_bytes());
    foreign.extend_from_slice(&42u32.to_le_bytes());
    r.load_persist(&foreign);
    assert_eq!(*r.active_tab.read().expect("lock"), 7);
}

#[test]
fn host_state_blob_includes_persist() {
    let p = TestParams::new();
    p.set_plain(1, -3.0);
    *p.ui_scale.write().expect("lock") = 1.25;
    *p.active_tab.write().expect("lock") = 3;
    let blob = encode_state(&p);
    assert!(blob.starts_with(b"AURA"));

    let q = TestParams::new();
    assert!(decode_state(&q, &blob));
    assert_eq!(q.get_plain(1), Some(-3.0));
    assert_eq!(*q.ui_scale.read().expect("lock"), 1.25);
    assert_eq!(*q.active_tab.read().expect("lock"), 3);
}

#[test]
fn param_enum_derive_surface() {
    assert_eq!(Mode::variant_count(), 3);
    assert_eq!(Mode::variant_names(), &["Clean", "Crunch+", "Dirt"]);
    assert_eq!(Mode::Crunch.name(), "Crunch+", "#[name] override");
    assert_eq!(Mode::Dirt.to_index(), 2);
    assert_eq!(Mode::from_index(2), Mode::Dirt);
    assert_eq!(
        Mode::from_index(99),
        Mode::Clean,
        "out-of-range falls to first"
    );
}

/// Parent declares `id = 10`, which the nested `SubParams` also
/// declares - the per-struct compile-time check can't see across
/// nested types, so `new()` must panic via `assert_no_id_collisions`.
#[derive(Params)]
struct CollidingParams {
    #[param(id = 10, name = "Clash", range = "linear(0, 1)")]
    clash: FloatParam,
    #[nested]
    sub: SubParams,
}

#[test]
#[should_panic(expected = "duplicate parameter ID 10")]
fn parent_nested_id_collision_panics_at_construction() {
    let _ = CollidingParams::new();
}

/// G15: `AudioTap` declared via the existing `#[skip]` mechanism (no
/// derive changes needed) - excluded from the automatable param
/// surface, reached by the editor through the concrete struct rather
/// than `dyn Params`.
#[derive(Params)]
struct TapParams {
    #[param(id = 1, name = "Gain", range = "linear(-24, 24)", default = 0.0)]
    gain: FloatParam,
    #[skip]
    spectrum_tap: AudioTap,
}

#[test]
fn audio_tap_skip_field_excluded_from_param_surface() {
    let p = TapParams::new();
    assert_eq!(p.count(), 1, "AudioTap is not a parameter");
    assert_eq!(p.param_infos().len(), 1);
    assert!(p.meter_ids().is_empty(), "AudioTap is not a meter either");
}

#[test]
fn audio_tap_concrete_field_access_round_trip() {
    let p = TapParams::new();
    // Audio-thread side: push raw samples every process() call.
    p.spectrum_tap.push(&[0.1, 0.2, 0.3]);
    // UI-thread side: the editor drains through the concrete
    // `Arc<Self::Params>` PluginLogic::editor() receives, not `dyn Params`.
    assert_eq!(p.spectrum_tap.drain(), vec![0.1, 0.2, 0.3]);
}

#[test]
fn param_id_enum_maps_explicit_ids() {
    use TestParamsParamId as P;

    // One variant per own param field; explicit `id = N` is carried.
    assert_eq!(P::Gain.id(), 1);
    assert_eq!(P::Octave.id(), 2);
    assert_eq!(P::Bypass.id(), 3);
    assert_eq!(P::Mode.id(), 4);
    assert_eq!(P::Cutoff.id(), 5);
    assert_eq!(u32::from(P::Gain), 1);

    // Nested params get their own enum; meters are excluded.
    assert_eq!(SubParamsParamId::Tone.id(), 10);
    assert_eq!(
        P::from_id(10),
        None,
        "nested IDs stay with SubParamsParamId"
    );

    // Round-trip every declared ID; unknown IDs fall out.
    for id in [1, 2, 3, 4, 5] {
        assert_eq!(P::from_id(id).map(P::id), Some(id));
    }
    assert_eq!(P::from_id(0), None);
    assert_eq!(P::from_id(u32::MAX), None);
}
