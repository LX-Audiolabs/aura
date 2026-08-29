//! Minimal aux-out FX — proves `BusLayout::with_aux` + `AudioBuffer::aux_output`
//! end-to-end through the format wrappers.
//!
//! Main outs = dry passthrough. Aux outs = amount * main (split send).
//!
//! ```bash
//! cargo aura install --clap --release -plug smoke-aux
//! ```

use aura::prelude::*;

#[derive(Params)]
pub struct AuxParams {
    #[param(id = 1, name = "Send", range = "linear(0, 1)", default = 1.0)]
    pub send: FloatParam,
}

pub struct SmokeAux;
pub struct DspState;

impl PluginLogic for SmokeAux {
    type Params = AuxParams;
    type DspState = DspState;

    fn info() -> PluginInfo {
        let mut info = PluginInfo::new(
            "AURA Smoke Aux",
            "LX Audiolabs",
            env!("CARGO_PKG_VERSION"),
            "smoke-aux",
        );
        info.clap_id = "com.lx-audiolabs.aura.smoke-aux";
        info.vst3_id = "com.lx-audiolabs.aura.smoke-aux";
        info.lv2_uri = "https://lx-audiolabs.com/lv2/smoke-aux";
        info.category = PluginCategory::Effect;
        info
    }

    fn bus_layouts() -> Vec<BusLayout> {
        vec![BusLayout::stereo().with_aux(ChannelConfig::Stereo)]
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
        #[allow(clippy::cast_possible_truncation)]
        let send = params.send.raw_target() as f32;
        let main_n = buffer.num_main_outputs().min(buffer.num_main_inputs());
        let aux_n = buffer.num_aux_outputs();

        // Copy main in → main out (read first in case of in-place).
        for c in 0..main_n {
            let input: Vec<f32> = buffer.main_input(c)[..n].to_vec();
            buffer.main_output(c)[..n].copy_from_slice(&input);
        }
        for c in main_n..buffer.num_main_outputs() {
            buffer.main_output(c)[..n].fill(0.0);
        }

        // Aux = send * corresponding main (or silence).
        for c in 0..aux_n {
            let src = if c < main_n {
                buffer.main_input(c)[..n].to_vec()
            } else {
                vec![0.0; n]
            };
            let out = buffer.aux_output(c);
            for i in 0..n {
                out[i] = src[i] * send;
            }
        }

        ProcessStatus::Continue
    }
}

#[cfg(feature = "clap")]
aura::export!(SmokeAux);

#[cfg(feature = "vst3")]
aura::export_vst3!(SmokeAux);

#[cfg(feature = "lv2")]
aura::export_lv2!(SmokeAux);
