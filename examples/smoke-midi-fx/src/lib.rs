//! Minimal MIDI FX — proves `context.midi` input + `context.midi_out` output
//! end-to-end through the format wrappers.
//!
//! Headless on purpose: `PluginLogic::editor` defaults to `None`, hosts show
//! their own generic parameter UI.
//!
//! ```bash
//! cargo build -p smoke-midi-fx --release --features clap
//! cargo run -p cargo-aura -- install --clap --release
//! # or copy target/release/smoke_midi_fx.dll → …/CLAP/smoke-midi-fx.clap
//! clap-validator validate path/to/smoke-midi-fx.clap
//!
//! # VST3:
//! cargo build -p smoke-midi-fx --release --features vst3
//! cargo run -p cargo-aura -- install --vst3 --release
//!
//! # LV2:
//! cargo build -p smoke-midi-fx --release --features lv2
//! cargo run -p cargo-aura -- install --lv2 --release
//! ```

use aura::prelude::*;

// ---------------------------------------------------------------------------
// Params
// ---------------------------------------------------------------------------

#[derive(Params)]
pub struct MidiFxParams {
    #[param(
        id = 1,
        name = "Transpose",
        range = "linear(-24, 24)",
        default = 12.0,
        unit = "st"
    )]
    pub transpose: FloatParam,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct SmokeMidiFx;

pub struct DspState;

impl PluginLogic for SmokeMidiFx {
    type Params = MidiFxParams;
    type DspState = DspState;

    fn info() -> PluginInfo {
        let mut info = PluginInfo::new(
            "AURA Smoke MIDI FX",
            "LX Audiolabs",
            env!("CARGO_PKG_VERSION"),
            "smoke-midi-fx",
        );
        info.clap_id = "com.lx-audiolabs.aura.smoke-midi-fx";
        info.vst3_id = "com.lx-audiolabs.aura.smoke-midi-fx";
        info.lv2_uri = "https://lx-audiolabs.com/lv2/smoke-midi-fx";
        info.category = PluginCategory::NoteEffect;
        info.accepts_midi_in = true;
        info.emits_midi = true;
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
        context: &mut ProcessContext,
    ) -> ProcessStatus {
        #[allow(clippy::cast_possible_truncation)]
        let transpose = params.transpose.raw_target() as i32;

        // Copy audio input → output unchanged (dry thru).
        let n = buffer.num_samples();
        let ch = buffer.num_outputs().min(buffer.num_inputs());
        for c in 0..ch {
            let input: Vec<f32> = buffer.input(c).to_vec();
            let out = buffer.output(c);
            out[..n].copy_from_slice(&input[..n]);
        }
        for c in ch..buffer.num_outputs() {
            buffer.output(c).fill(0.0);
        }

        // Transpose note messages and forward everything else as-is.
        for ev in context.midi.iter() {
            let msg = ev.message;
            let out_msg = if msg.is_note_on() || msg.is_note_off() {
                let note = msg
                    .note_number()
                    .map(|n| (n as i32 + transpose).clamp(0, 127) as u8)
                    .unwrap_or(msg.data1);
                MidiMessage::raw(msg.status_byte(), note, msg.data2)
            } else {
                msg
            };
            context.midi_out.push(ev.sample_offset, out_msg);
        }

        ProcessStatus::Continue
    }
}

#[cfg(feature = "clap")]
aura::export!(SmokeMidiFx);

#[cfg(feature = "vst3")]
aura::export_vst3!(SmokeMidiFx);

#[cfg(feature = "lv2")]
aura::export_lv2!(SmokeMidiFx);
