//! Minimal monophonic synth — proves `context.midi` + `aura-dsp` voice path
//! (Oscillator + Adsr) end-to-end through the format wrappers.
//!
//! Headless on purpose: `PluginLogic::editor` defaults to `None`, hosts show
//! their own generic parameter UI.
//!
//! ```bash
//! cargo build -p smoke-synth --release --features clap
//! cargo run -p cargo-aura -- install --clap --release
//! # or copy target/release/smoke_synth.dll → …/CLAP/smoke-synth.clap
//! clap-validator validate path/to/smoke-synth.clap
//!
//! # VST3:
//! cargo build -p smoke-synth --release --features vst3
//! cargo run -p cargo-aura -- install --vst3 --release
//! ```

use aura::dsp::envelope::Adsr;
use aura::dsp::oscillator::{Oscillator, Waveform};
use aura::midi::midi_note_to_freq;
use aura::prelude::*;

// Fixed voice envelope (seconds / level) — smoke test, not a product synth.
const ATTACK: f32 = 0.005;
const DECAY: f32 = 0.05;
const SUSTAIN: f32 = 0.8;
const RELEASE: f32 = 0.1;

// ---------------------------------------------------------------------------
// Params
// ---------------------------------------------------------------------------

#[derive(Params)]
pub struct SynthParams {
    #[param(
        id = 1,
        name = "Gain",
        range = "linear(-24, 24)",
        default = 0.0,
        unit = "db"
    )]
    pub gain: FloatParam,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct SmokeSynth;

pub struct DspState {
    osc: Oscillator,
    env: Adsr,
}

fn make_osc(freq: f32, sample_rate: f32) -> Oscillator {
    Oscillator::new(Waveform::Sine, freq, sample_rate)
        .unwrap_or_else(|_| Oscillator::new(Waveform::Sine, 440.0, 44_100.0).unwrap())
}

fn make_env(sample_rate: f32) -> Adsr {
    Adsr::with_sample_rate(ATTACK, DECAY, SUSTAIN, RELEASE, sample_rate)
        .unwrap_or_else(|_| Adsr::new(ATTACK, DECAY, SUSTAIN, RELEASE).unwrap())
}

impl PluginLogic for SmokeSynth {
    type Params = SynthParams;
    type DspState = DspState;

    fn info() -> PluginInfo {
        let mut info = PluginInfo::new(
            "AURA Smoke Synth",
            "LX Audiolabs",
            env!("CARGO_PKG_VERSION"),
            "smoke-synth",
        );
        info.clap_id = "com.lx-audiolabs.aura.smoke-synth";
        // Stable once shipped — host sessions key off this string → TUID.
        info.vst3_id = "com.lx-audiolabs.aura.smoke-synth";
        // Must match cargo-aura's LV2 fallback TTL (derived from package name)
        // until a build-time TTL sidecar exists — host scans TTL, then matches
        // lv2_descriptor's URI against it.
        info.lv2_uri = "https://lx-audiolabs.com/lv2/smoke-synth";
        info.category = PluginCategory::Instrument;
        info.accepts_midi_in = true;
        info
    }

    fn bus_layouts() -> Vec<BusLayout> {
        // Instrument: no audio input, mono voice copied to all outputs.
        vec![
            BusLayout::output_only(ChannelConfig::Stereo),
            BusLayout::output_only(ChannelConfig::Mono),
        ]
    }

    fn init(_params: &Self::Params, sample_rate: f64) -> Self::DspState {
        #[allow(clippy::cast_possible_truncation)]
        let sr = sample_rate as f32;
        DspState {
            osc: make_osc(440.0, sr),
            env: make_env(sr),
        }
    }

    fn reset(state: &mut Self::DspState, _params: &Self::Params, config: &AudioConfig) {
        #[allow(clippy::cast_possible_truncation)]
        let sr = config.sample_rate as f32;
        // Rebuild both at the (possibly new) sample rate; keep current pitch.
        state.osc = make_osc(state.osc.frequency(), sr);
        state.env = make_env(sr);
    }

    fn process(
        state: &mut Self::DspState,
        params: &Self::Params,
        buffer: &mut AudioBuffer<'_, f32>,
        context: &mut ProcessContext,
    ) -> ProcessStatus {
        // Block-accurate note handling is fine for the smoke test (last note
        // wins, monophonic); sample_offset is intentionally ignored.
        for ev in context.midi.iter() {
            let msg = ev.message;
            if msg.is_note_on() {
                if let Some(note) = msg.note_number() {
                    let _ = state.osc.set_frequency(midi_note_to_freq(note));
                    state.env.gate_on();
                }
            } else if msg.is_note_off() {
                state.env.gate_off();
            }
        }

        let n = buffer.num_samples();
        let outs = buffer.num_outputs();
        #[allow(clippy::cast_possible_truncation)]
        let gain_db = params.gain.raw_target() as f32;
        let lin = 10.0f32.powf(gain_db / 20.0);

        for i in 0..n {
            let s = state.osc.next_sample() * state.env.next_value() * lin;
            for c in 0..outs {
                buffer.output(c)[i] = s;
            }
        }
        ProcessStatus::Continue
    }
}

#[cfg(feature = "clap")]
aura::export!(SmokeSynth);

#[cfg(feature = "vst3")]
aura::export_vst3!(SmokeSynth);

#[cfg(feature = "lv2")]
aura::export_lv2!(SmokeSynth);
