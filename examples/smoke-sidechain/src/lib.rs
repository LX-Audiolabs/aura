//! Minimal sidechain FX — proves `AudioBuffer::sidechain_input` end-to-end
//! through the format wrappers.
//!
//! Headless on purpose: `PluginLogic::editor` defaults to `None`, hosts show
//! their own generic parameter UI.
//!
//! ```bash
//! cargo build -p smoke-sidechain --release --features clap
//! cargo run -p cargo-aura -- install --clap --release
//! # or copy target/release/smoke_sidechain.dll → …/CLAP/smoke-sidechain.clap
//! clap-validator validate path/to/smoke-sidechain.clap
//!
//! # VST3:
//! cargo build -p smoke-sidechain --release --features vst3
//! cargo run -p cargo-aura -- install --vst3 --release
//!
//! # LV2:
//! cargo build -p smoke-sidechain --release --features lv2
//! cargo run -p cargo-aura -- install --lv2 --release
//! ```

use aura::prelude::*;

// ---------------------------------------------------------------------------
// Params
// ---------------------------------------------------------------------------

#[derive(Params)]
pub struct SidechainParams {
    #[param(id = 1, name = "Amount", range = "linear(0, 1)", default = 0.5)]
    pub amount: FloatParam,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct SmokeSidechain;

pub struct DspState;

impl PluginLogic for SmokeSidechain {
    type Params = SidechainParams;
    type DspState = DspState;

    fn info() -> PluginInfo {
        let mut info = PluginInfo::new(
            "AURA Smoke Sidechain",
            "LX Audiolabs",
            env!("CARGO_PKG_VERSION"),
            "smoke-sidechain",
        );
        info.clap_id = "com.lx-audiolabs.aura.smoke-sidechain";
        info.vst3_id = "com.lx-audiolabs.aura.smoke-sidechain";
        info.lv2_uri = "https://lx-audiolabs.com/lv2/smoke-sidechain";
        info.category = PluginCategory::Effect;
        info
    }

    fn bus_layouts() -> Vec<BusLayout> {
        // Stereo main I/O with a mono sidechain input.
        vec![BusLayout::stereo().with_sidechain(ChannelConfig::Mono)]
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
        let outs = buffer.num_outputs();
        #[allow(clippy::cast_possible_truncation)]
        let amount = params.amount.raw_target() as f32;

        // Sidechain is declared mono; read it once and copy to all outputs.
        let sc = if buffer.num_sidechain_inputs() > 0 {
            buffer.sidechain_input(0).to_vec()
        } else {
            vec![0.0; n]
        };

        for c in 0..outs {
            let main = if c < buffer.num_main_inputs() {
                buffer.main_input(c).to_vec()
            } else {
                vec![0.0; n]
            };
            let out = buffer.output(c);
            for i in 0..n {
                out[i] = main[i] * (1.0 - amount) + sc[i] * amount;
            }
        }

        ProcessStatus::Continue
    }
}

#[cfg(feature = "clap")]
aura::export!(SmokeSidechain);

#[cfg(feature = "vst3")]
aura::export_vst3!(SmokeSidechain);

#[cfg(feature = "lv2")]
aura::export_lv2!(SmokeSidechain);
