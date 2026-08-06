//! Minimal stereo gain — proves `aura-clap` + `PluginLogic` end-to-end.
//!
//! ```bash
//! cargo build -p smoke-gain --release
//! cargo run -p cargo-aura -- install --clap --release
//! # or copy target/release/smoke_gain.dll → …/CLAP/smoke-gain.clap
//! clap-validator validate path/to/smoke-gain.clap
//! ```

use std::sync::Arc;

use aura::prelude::*;
use aura_params::{
    FloatParam, ParamFlags, ParamInfo, ParamRange, ParamUnit, ParamValueKind, Params,
    SmoothingStyle, __private::Sealed, format_param_value,
};

slint::include_modules!();

const GAIN_ID: u32 = 1;

// ---------------------------------------------------------------------------
// Params (hand-written until aura-derive)
// ---------------------------------------------------------------------------

pub struct GainParams {
    pub gain: FloatParam,
}

impl Default for GainParams {
    fn default() -> Self {
        let info = ParamInfo {
            id: GAIN_ID,
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
        };
        Self {
            gain: FloatParam::new(info, SmoothingStyle::None),
        }
    }
}

impl Sealed for GainParams {}

impl Params for GainParams {
    fn param_infos(&self) -> Vec<ParamInfo> {
        vec![self.gain.info]
    }

    fn count(&self) -> usize {
        1
    }

    fn get_normalized(&self, id: u32) -> Option<f64> {
        if id != GAIN_ID {
            return None;
        }
        Some(self.gain.info.range.normalize(self.gain.raw_target()))
    }

    fn set_normalized(&self, id: u32, value: f64) {
        if id != GAIN_ID {
            return;
        }
        let plain = self.gain.info.range.denormalize(value);
        self.gain.set_value(plain);
    }

    fn get_plain(&self, id: u32) -> Option<f64> {
        if id != GAIN_ID {
            return None;
        }
        Some(self.gain.raw_target())
    }

    fn set_plain(&self, id: u32, value: f64) {
        if id != GAIN_ID {
            return;
        }
        self.gain.set_value(value);
    }

    fn format_value(&self, id: u32, value: f64) -> Option<String> {
        if id != GAIN_ID {
            return None;
        }
        Some(format_param_value(&self.gain.info, value))
    }

    fn parse_value(&self, id: u32, text: &str) -> Option<f64> {
        if id != GAIN_ID {
            return None;
        }
        text.trim()
            .trim_end_matches("dB")
            .trim_end_matches("db")
            .trim()
            .parse()
            .ok()
    }

    fn snap_smoothers(&self) {
        self.gain.smoother.snap(self.gain.raw_target());
    }

    fn set_sample_rate(&self, sample_rate: f64) {
        self.gain.smoother.set_sample_rate(sample_rate);
    }

    fn collect_values(&self) -> (Vec<u32>, Vec<f64>) {
        (vec![GAIN_ID], vec![self.gain.raw_target()])
    }

    fn restore_values(&self, values: &[(u32, f64)]) {
        for &(id, v) in values {
            self.set_plain(id, v);
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct SmokeGain;

pub struct DspState;

impl PluginLogic for SmokeGain {
    type Params = GainParams;
    type DspState = DspState;

    fn info() -> PluginInfo {
        let mut info = PluginInfo::new(
            "AURA Smoke Gain",
            "LX Audiolabs",
            env!("CARGO_PKG_VERSION"),
            "smoke-gain",
        );
        info.clap_id = "com.lx-audiolabs.aura.smoke-gain";
        info.category = PluginCategory::Effect;
        info
    }

    fn init(_params: &Self::Params, _sample_rate: f64) -> Self::DspState {
        DspState
    }

    fn reset(_state: &mut Self::DspState, _params: &Self::Params, _config: &AudioConfig) {}

    fn process(
        _state: &mut Self::DspState,
        params: &Self::Params,
        buffer: &mut AudioBuffer<'_, f32>,
        _context: &mut ProcessContext,
    ) -> ProcessStatus {
        let n = buffer.num_samples();
        let gain_db = params.gain.raw_target() as f32;
        let lin = 10.0f32.powf(gain_db / 20.0);

        let ch = buffer.num_outputs().min(buffer.num_inputs());
        for c in 0..ch {
            // Copy input → output with gain (handle possible aliasing by reading first).
            let input: Vec<f32> = buffer.input(c).to_vec();
            let out = buffer.output(c);
            for i in 0..n {
                out[i] = input[i] * lin;
            }
        }
        // Extra outs: silence
        for c in ch..buffer.num_outputs() {
            buffer.output(c).fill(0.0);
        }
        ProcessStatus::Continue
    }

    fn editor(_params: Arc<Self::Params>) -> Option<Box<dyn Editor>> {
        Some(
            aura_editor::AuraSlintEditor::new(
                (320, 200),
                |ctx| {
                    let ui = AppWindow::new().expect("slint component");
                    let params = ctx.params.clone();
                    ui.on_gain_changed(move |v| params.set_plain(GAIN_ID, f64::from(v)));
                    ui
                },
                |ui, ctx| {
                    #[allow(clippy::cast_possible_truncation)]
                    let v = ctx.params.get_plain(GAIN_ID).unwrap_or(0.0) as f32;
                    // Guard: don't fight an active drag with per-frame sync.
                    if (v - ui.get_gain()).abs() > 1.0e-4 {
                        ui.set_gain(v);
                    }
                },
            )
            .into_editor(),
        )
    }
}

#[cfg(feature = "clap")]
aura::export!(SmokeGain);
