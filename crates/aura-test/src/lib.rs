//! Test helpers for AURA plugins.
//!
//! Drop-in spirit of product `truce_test` for the AURA cutover: state
//! round-trip, param sanity, and a small offline process harness.
//! No host / CLAP load — pure `PluginLogic`.
//!
//! ```ignore
//! #[test]
//! fn state_round_trips() {
//!     aura_test::assert_state_round_trip::<MyPlugin>();
//! }
//!
//! #[test]
//! fn process_is_finite() {
//!     let out = aura_test::process_silence::<MyPlugin>(2, 256);
//!     aura_test::assert_no_nans(&out);
//! }
//! ```

#![forbid(unsafe_code)]
// Assertion helpers panic on failure by design — same contract as product tests.
#![allow(clippy::missing_panics_doc, clippy::must_use_candidate)]

use aura_core::{
    AudioBuffer, AudioConfig, PluginLogic, ProcessContext, ProcessMode, ProcessStatus,
    decode_state, encode_state,
};
use aura_params::Params;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Encode → fresh instance → decode; every param plain value must match.
///
/// # Panics
///
/// On decode failure, missing param id, or |Δ| ≥ 1e-4.
pub fn assert_state_round_trip<L: PluginLogic>() {
    let src = L::Params::default();
    let blob = encode_state(&src);

    let dst = L::Params::default();
    assert!(
        decode_state(&dst, &blob),
        "decode_state failed on a blob produced by encode_state — codec bug"
    );

    let infos = src.param_infos();
    for pi in &infos {
        let v1 = src.get_plain(pi.id).unwrap_or_else(|| {
            panic!(
                "param {} ({}) missing from source after encode — id not registered",
                pi.id, pi.name
            )
        });
        let v2 = dst.get_plain(pi.id).unwrap_or_else(|| {
            panic!(
                "param {} ({}) lost during state round-trip — \
                 blob id not restored on a fresh Default params",
                pi.id, pi.name
            )
        });
        assert!(
            (v1 - v2).abs() < 1e-4,
            "param {} ({}) mismatch after round-trip: {v1} vs {v2}",
            pi.id,
            pi.name
        );
    }
}

/// Corrupt / truncated blobs must not panic and must leave defaults intact.
///
/// # Panics
///
/// If `decode_state` panics, or if a failed decode still mutates values.
pub fn assert_corrupt_state_no_crash<L: PluginLogic>() {
    let params = L::Params::default();
    let before = snapshot_plains(&params);

    // Truncated / random / empty
    for blob in [
        &[][..],
        &[0u8, 1, 2, 3][..],
        &[0xff; 7][..],
        // Lies about count
        &[10, 0, 0, 0, 1, 0, 0, 0][..],
    ] {
        let ok = decode_state(&params, blob);
        assert!(
            !ok || blob.is_empty(),
            "unexpected success on corrupt blob {blob:?}"
        );
        let after = snapshot_plains(&params);
        assert_eq!(
            before, after,
            "failed decode must not mutate param values (blob {blob:?})"
        );
    }
}

/// Empty blob is a no-op reject (same as corrupt).
pub fn assert_empty_state_no_crash<L: PluginLogic>() {
    let params = L::Params::default();
    let before = snapshot_plains(&params);
    assert!(!decode_state(&params, &[]));
    assert_eq!(before, snapshot_plains(&params));
}

fn snapshot_plains(params: &dyn Params) -> Vec<(u32, f64)> {
    let infos = params.param_infos();
    infos
        .iter()
        .filter_map(|pi| params.get_plain(pi.id).map(|v| (pi.id, v)))
        .collect()
}

// ---------------------------------------------------------------------------
// Params shape
// ---------------------------------------------------------------------------

/// No two infos share the same id.
///
/// # Panics
///
/// On duplicate ids.
pub fn assert_no_duplicate_param_ids<L: PluginLogic>() {
    let infos = L::Params::default().param_infos();
    let mut seen = Vec::with_capacity(infos.len());
    for pi in &infos {
        assert!(
            !seen.contains(&pi.id),
            "duplicate param id {} ({})",
            pi.id,
            pi.name
        );
        seen.push(pi.id);
    }
}

/// `count()` matches `param_infos().len()`.
pub fn assert_param_count_matches<L: PluginLogic>() {
    let p = L::Params::default();
    assert_eq!(
        p.count(),
        p.param_infos().len(),
        "Params::count() != param_infos().len()"
    );
}

/// Defaults match `ParamInfo::default_plain` within 1e-9.
pub fn assert_param_defaults_match<L: PluginLogic>() {
    let p = L::Params::default();
    for pi in p.param_infos() {
        let v = p
            .get_plain(pi.id)
            .unwrap_or_else(|| panic!("param {} ({}) has no get_plain", pi.id, pi.name));
        assert!(
            (v - pi.default_plain).abs() < 1e-9,
            "param {} ({}) default mismatch: value={v} info={}",
            pi.id,
            pi.name,
            pi.default_plain
        );
    }
}

/// Plugin info strings are non-empty.
pub fn assert_valid_info<L: PluginLogic>() {
    let info = L::info();
    assert!(!info.name.is_empty(), "PluginInfo.name is empty");
    assert!(!info.vendor.is_empty(), "PluginInfo.vendor is empty");
    assert!(!info.bundle_id.is_empty(), "PluginInfo.bundle_id is empty");
    assert!(!info.clap_id.is_empty(), "PluginInfo.clap_id is empty");
}

// ---------------------------------------------------------------------------
// Process smoke (offline, no host)
// ---------------------------------------------------------------------------

/// Run one process block of silence; return per-channel output buffers.
///
/// Uses the first bus layout’s main channel count (default stereo).
///
/// # Panics
///
/// If process panics (not fenced — test harness wants the panic).
pub fn process_silence<L: PluginLogic>(channels: usize, frames: usize) -> Vec<Vec<f32>> {
    process_with_input::<L>(&vec![vec![0.0f32; frames]; channels], frames)
}

/// Process one block from owned input channels (same length).
pub fn process_with_input<L: PluginLogic>(inputs: &[Vec<f32>], frames: usize) -> Vec<Vec<f32>> {
    let params = L::Params::default();
    let sample_rate = 44_100.0;
    let mut state = L::init(&params, sample_rate);
    let layout = L::bus_layouts().into_iter().next();
    let ch_in = layout.map_or(inputs.len(), |l| l.main_input_channels() as usize);
    let ch_out = layout
        .map_or(inputs.len(), |l| l.main_output_channels() as usize)
        .max(1);
    #[allow(clippy::cast_possible_truncation)] // channel counts are tiny
    let config = AudioConfig::new(sample_rate, frames).with_channels(ch_in as u32, ch_out as u32);
    L::reset(&mut state, &params, &config);

    let owned_in: Vec<Vec<f32>> = (0..ch_out)
        .map(|c| inputs.get(c).cloned().unwrap_or_else(|| vec![0.0; frames]))
        .collect();
    let mut owned_out = owned_in.clone();

    let in_refs: Vec<&[f32]> = owned_in.iter().map(Vec::as_slice).collect();
    let mut out_refs: Vec<&mut [f32]> = owned_out.iter_mut().map(Vec::as_mut_slice).collect();
    let mut buffer = AudioBuffer::from_slices_checked(&in_refs, &mut out_refs, frames);
    let mut ctx = ProcessContext::new(sample_rate, frames).with_process_mode(ProcessMode::Realtime);
    let status = L::process(&mut state, &params, &mut buffer, &mut ctx);
    assert!(
        matches!(
            status,
            ProcessStatus::Continue | ProcessStatus::TailFinished
        ),
        "process returned Error"
    );
    owned_out
}

// ---------------------------------------------------------------------------
// Buffer assertions
// ---------------------------------------------------------------------------

const AUDIBLE: f32 = 1e-3;

/// Every sample is finite (no NaN / Inf).
///
/// # Panics
///
/// On first non-finite sample.
pub fn assert_no_nans(channels: &[Vec<f32>]) {
    for (ci, ch) in channels.iter().enumerate() {
        for (i, s) in ch.iter().enumerate() {
            assert!(s.is_finite(), "non-finite sample at ch={ci} i={i}: {s}");
        }
    }
}

/// At least one |sample| > 1e-3.
pub fn assert_nonzero(channels: &[Vec<f32>]) {
    let peak = channels
        .iter()
        .flat_map(|c| c.iter())
        .map(|s| s.abs())
        .fold(0.0f32, f32::max);
    assert!(
        peak > AUDIBLE,
        "output is silent (peak={peak}); expected audible content"
    );
}

/// Every |sample| ≤ 1e-3.
pub fn assert_silence(channels: &[Vec<f32>]) {
    let peak = channels
        .iter()
        .flat_map(|c| c.iter())
        .map(|s| s.abs())
        .fold(0.0f32, f32::max);
    assert!(peak <= AUDIBLE, "output not silent (peak={peak})");
}

// ---------------------------------------------------------------------------
// Self-tests (minimal PluginLogic)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use aura_core::{
        AudioBuffer, AudioConfig, Editor, PluginInfo, PluginLogic, ProcessContext, ProcessStatus,
    };
    use aura_params::{
        FloatParam, ParamFlags, ParamInfo, ParamRange, ParamUnit, ParamValueKind, Params,
    };

    use super::*;

    struct GainParams {
        gain: FloatParam,
    }

    impl aura_params::__private::Sealed for GainParams {}

    impl Default for GainParams {
        fn default() -> Self {
            Self {
                gain: FloatParam::new(
                    ParamInfo {
                        id: 1,
                        name: "Gain",
                        short_name: "Gain",
                        group: "",
                        range: ParamRange::Linear {
                            min: -24.0,
                            max: 24.0,
                        },
                        default_plain: 0.0,
                        flags: ParamFlags::AUTOMATABLE,
                        unit: ParamUnit::Db,
                        kind: ParamValueKind::Float,
                        midi_map: None,
                        midi_channel: None,
                    },
                    aura_params::SmoothingStyle::None,
                ),
            }
        }
    }

    impl Params for GainParams {
        fn param_infos(&self) -> Vec<ParamInfo> {
            vec![self.gain.info]
        }
        fn count(&self) -> usize {
            1
        }
        fn get_plain(&self, id: u32) -> Option<f64> {
            (id == 1).then(|| self.gain.raw_target())
        }
        fn set_plain(&self, id: u32, value: f64) {
            if id == 1 {
                self.gain.set_value(value);
            }
        }
        fn get_normalized(&self, id: u32) -> Option<f64> {
            self.get_plain(id)
                .map(|v| self.gain.info.range.normalize(v))
        }
        fn set_normalized(&self, id: u32, n: f64) {
            if id == 1 {
                let p = self.gain.info.range.denormalize(n);
                self.gain.set_value(p);
            }
        }
        fn set_sample_rate(&self, _sr: f64) {}
        fn snap_smoothers(&self) {}
        fn collect_values(&self) -> (Vec<u32>, Vec<f64>) {
            (vec![1], vec![self.gain.raw_target()])
        }
        fn restore_values(&self, values: &[(u32, f64)]) {
            for &(id, v) in values {
                self.set_plain(id, v);
            }
        }
        fn format_value(&self, id: u32, plain: f64) -> Option<String> {
            (id == 1).then(|| format!("{plain:.1}"))
        }
        fn parse_value(&self, id: u32, text: &str) -> Option<f64> {
            (id == 1).then(|| text.parse().ok()).flatten()
        }
    }

    struct GainPlug;
    struct GainState;

    impl PluginLogic for GainPlug {
        type Params = GainParams;
        type DspState = GainState;

        fn info() -> PluginInfo {
            PluginInfo::new("Gain", "Test", "0.1.0", "gain")
        }

        fn init(_params: &Self::Params, _sr: f64) -> Self::DspState {
            GainState
        }

        fn reset(_s: &mut Self::DspState, _p: &Self::Params, _c: &AudioConfig) {}

        fn process(
            _s: &mut Self::DspState,
            params: &Self::Params,
            buffer: &mut AudioBuffer<'_, f32>,
            _ctx: &mut ProcessContext,
        ) -> ProcessStatus {
            #[allow(clippy::cast_possible_truncation)]
            let g = 10.0f32.powf(params.gain.raw_target() as f32 / 20.0);
            let n = buffer.num_samples();
            let ch = buffer.num_outputs().min(buffer.num_inputs());
            for c in 0..ch {
                let input: Vec<f32> = buffer.input(c).to_vec();
                let out = buffer.output(c);
                for i in 0..n {
                    out[i] = input[i] * g;
                }
            }
            ProcessStatus::Continue
        }

        fn editor(_p: Arc<Self::Params>) -> Option<Box<dyn Editor>> {
            None
        }
    }

    #[test]
    fn state_round_trip_and_shape() {
        assert_valid_info::<GainPlug>();
        assert_no_duplicate_param_ids::<GainPlug>();
        assert_param_count_matches::<GainPlug>();
        assert_param_defaults_match::<GainPlug>();
        assert_state_round_trip::<GainPlug>();
        assert_corrupt_state_no_crash::<GainPlug>();
        assert_empty_state_no_crash::<GainPlug>();
    }

    #[test]
    fn process_silence_is_finite_and_silent() {
        let out = process_silence::<GainPlug>(2, 64);
        assert_eq!(out.len(), 2);
        assert_no_nans(&out);
        assert_silence(&out);
    }

    #[test]
    fn process_constant_is_nonzero() {
        let frames = 32;
        let input = vec![vec![0.5f32; frames]; 2];
        let out = process_with_input::<GainPlug>(&input, frames);
        assert_no_nans(&out);
        assert_nonzero(&out);
    }
}
