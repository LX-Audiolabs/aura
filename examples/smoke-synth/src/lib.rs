//! Minimal monophonic synth — proves `context.notes` (CLAP note-id,
//! expressions, per-note `PARAM_MOD`) plus MIDI fallback + `aura-dsp`
//! (Oscillator + Adsr) through the format wrappers.
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
        unit = "db",
        flags = "automatable | modulatable | modulatable_per_note"
    )]
    pub gain: FloatParam,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct SmokeSynth;

pub struct DspState {
    osc_sine: Oscillator,
    osc_saw: Oscillator,
    env: Adsr,
    note_id: i32,
    key: i16,
    tuning_semis: f32,
    /// CLAP note-on velocity 0..1.
    velocity: f32,
    /// CLAP volume expression: 0..4, 1 = 0 dB (Bitwig "Gain").
    volume: f32,
    /// CLAP brightness 0..1 (Bitwig "Timbre"): sine → saw.
    timbre: f32,
    pressure: f32,
    gain_mod: f64,
    /// Per-note absolute Gain (`PARAM_VALUE`); `None` = use knob + mod.
    gain_plain: Option<f64>,
    voices: NoteVoiceTable,
}

fn make_osc(wave: Waveform, freq: f32, sample_rate: f32) -> Oscillator {
    Oscillator::new(wave, freq, sample_rate)
        .unwrap_or_else(|_| Oscillator::new(wave, 440.0, 44_100.0).unwrap())
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
        // Prefer native CLAP notes so Bitwig sends expressions + per-note mod.
        info.midi_input_dialect = aura::MidiDialect::Clap;
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
            osc_sine: make_osc(Waveform::Sine, 440.0, sr),
            osc_saw: make_osc(Waveform::Saw, 440.0, sr),
            env: make_env(sr),
            note_id: -1,
            key: 69,
            tuning_semis: 0.0,
            velocity: 1.0,
            volume: 1.0,
            timbre: 0.0,
            pressure: 1.0,
            gain_mod: 0.0,
            gain_plain: None,
            voices: NoteVoiceTable::new(1),
        }
    }

    fn reset(state: &mut Self::DspState, _params: &Self::Params, config: &AudioConfig) {
        #[allow(clippy::cast_possible_truncation)]
        let sr = config.sample_rate as f32;
        let hz = state.osc_sine.frequency();
        state.osc_sine = make_osc(Waveform::Sine, hz, sr);
        state.osc_saw = make_osc(Waveform::Saw, hz, sr);
        state.env = make_env(sr);
    }

    fn process(
        state: &mut Self::DspState,
        params: &Self::Params,
        buffer: &mut AudioBuffer<'_, f32>,
        context: &mut ProcessContext,
    ) -> ProcessStatus {
        if context.notes.is_empty() {
            // VST3 / LV2 / MIDI-only hosts — last note wins, monophonic.
            for ev in context.midi.iter() {
                let msg = ev.message;
                if msg.is_note_on() {
                    if let Some(note) = msg.note_number() {
                        state.key = i16::from(note);
                        state.note_id = -1;
                        reset_voice_expr(state);
                        state.velocity = f32::from(msg.data2) / 127.0;
                        retune(state);
                        state.env.gate_on();
                    }
                } else if msg.is_note_off() {
                    state.env.gate_off();
                }
            }
        } else {
            state.voices.apply(&context.notes);
            // Note-on first so Bitwig expressions in the same block still apply.
            for ev in context.notes.iter() {
                if matches!(ev.kind, NoteEventKind::On { .. }) {
                    apply_note(state, ev);
                }
            }
            for ev in context.notes.iter() {
                if !matches!(ev.kind, NoteEventKind::On { .. }) {
                    apply_note(state, ev);
                }
            }
        }

        let n = buffer.num_samples();
        let outs = buffer.num_outputs();
        let gain_plain = state
            .gain_plain
            .unwrap_or_else(|| params.gain.effective_target());
        #[allow(clippy::cast_possible_truncation)]
        let gain_db = (gain_plain + state.gain_mod) as f32;
        let lin = 10.0f32.powf(gain_db / 20.0)
            * state.velocity.clamp(0.0, 1.0)
            * state.volume.clamp(0.0, 4.0)
            * state.pressure.clamp(0.0, 1.0);
        let mix = state.timbre.clamp(0.0, 1.0);

        for i in 0..n {
            let sine = state.osc_sine.next_sample();
            let saw = state.osc_saw.next_sample();
            let s = (sine * (1.0 - mix) + saw * mix) * state.env.next_value() * lin;
            for c in 0..outs {
                buffer.output(c)[i] = s;
            }
        }
        if !state.env.is_active() {
            #[allow(clippy::cast_possible_truncation)]
            let end_at = n.saturating_sub(1) as u32;
            state.voices.mark_all_silent(end_at);
        }
        state.voices.flush_ends(&mut context.notes_out);
        ProcessStatus::Continue
    }
}

/// CLAP velocities/expressions are 0..1. Tolerate 0..127 from sloppy hosts.
fn clap_unit(v: f64) -> f32 {
    #[allow(clippy::cast_possible_truncation)]
    if v > 1.0 {
        (v / 127.0).clamp(0.0, 1.0) as f32
    } else {
        v.clamp(0.0, 1.0) as f32
    }
}

fn reset_voice_expr(state: &mut DspState) {
    state.tuning_semis = 0.0;
    state.velocity = 1.0;
    state.volume = 1.0;
    state.timbre = 0.0;
    state.pressure = 1.0;
    state.gain_mod = 0.0;
    state.gain_plain = None;
}

fn retune(state: &mut DspState) {
    let key = u8::try_from(state.key.clamp(0, 127)).unwrap_or(69);
    let hz = midi_note_to_freq(key) * 2.0f32.powf(state.tuning_semis / 12.0);
    let _ = state.osc_sine.set_frequency(hz);
    let _ = state.osc_saw.set_frequency(hz);
}

fn apply_note(state: &mut DspState, ev: NoteEvent) {
    match ev.kind {
        NoteEventKind::On { velocity } => {
            state.note_id = ev.note_id;
            state.key = ev.key;
            reset_voice_expr(state);
            state.velocity = clap_unit(velocity);
            retune(state);
            state.env.gate_on();
        }
        NoteEventKind::Off { .. } | NoteEventKind::Choke => {
            if ev.matches_voice(state.note_id, state.key) {
                state.env.gate_off();
            }
        }
        NoteEventKind::End => {}
        NoteEventKind::Expression { id, value } => {
            if !ev.matches_voice(state.note_id, state.key) {
                return;
            }
            #[allow(clippy::cast_possible_truncation)]
            let v = value as f32;
            match id {
                NoteExpression::Tuning => {
                    state.tuning_semis = v;
                    retune(state);
                }
                NoteExpression::Volume => state.volume = v,
                NoteExpression::Brightness | NoteExpression::Expression => state.timbre = v,
                NoteExpression::Pressure => state.pressure = v,
                _ => {}
            }
        }
        NoteEventKind::ParamMod { param_id, amount } => {
            if param_id == 1 && ev.matches_voice(state.note_id, state.key) {
                state.gain_mod = amount;
            }
        }
        NoteEventKind::ParamValue { param_id, plain } => {
            if param_id == 1 && ev.matches_voice(state.note_id, state.key) {
                state.gain_plain = Some(plain);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_drive_tuning_and_poly_mod() {
        let params = SynthParams::default();
        let mut state = SmokeSynth::init(&params, 44_100.0);
        apply_note(&mut state, NoteEvent::on(0, 3, 60, 0.8));
        assert!((state.velocity - 0.8).abs() < 1e-6);
        apply_note(
            &mut state,
            NoteEvent {
                sample_offset: 1,
                note_id: 3,
                port_index: 0,
                channel: 0,
                key: 60,
                kind: NoteEventKind::Expression {
                    id: NoteExpression::Tuning,
                    value: 12.0,
                },
            },
        );
        let expect = midi_note_to_freq(60) * 2.0;
        assert!((state.osc_sine.frequency() - expect).abs() < 0.05);
        apply_note(
            &mut state,
            NoteEvent {
                sample_offset: 1,
                note_id: 3,
                port_index: 0,
                channel: 0,
                key: 60,
                kind: NoteEventKind::Expression {
                    id: NoteExpression::Volume,
                    value: 0.5,
                },
            },
        );
        assert!((state.volume - 0.5).abs() < 1e-6);
        apply_note(
            &mut state,
            NoteEvent {
                sample_offset: 1,
                note_id: 3,
                port_index: 0,
                channel: 0,
                key: 60,
                kind: NoteEventKind::Expression {
                    id: NoteExpression::Brightness,
                    value: 0.75,
                },
            },
        );
        assert!((state.timbre - 0.75).abs() < 1e-6);
        apply_note(
            &mut state,
            NoteEvent {
                sample_offset: 2,
                note_id: 3,
                port_index: 0,
                channel: 0,
                key: 60,
                kind: NoteEventKind::ParamMod {
                    param_id: 1,
                    amount: 6.0,
                },
            },
        );
        assert!((state.gain_mod - 6.0).abs() < 1e-12);
        // Other note_id must not steal the voice.
        apply_note(
            &mut state,
            NoteEvent {
                sample_offset: 3,
                note_id: 99,
                port_index: 0,
                channel: 0,
                key: 64,
                kind: NoteEventKind::ParamMod {
                    param_id: 1,
                    amount: -12.0,
                },
            },
        );
        assert!((state.gain_mod - 6.0).abs() < 1e-12);
    }
}

#[cfg(feature = "clap")]
aura::export!(SmokeSynth);

#[cfg(feature = "vst3")]
aura::export_vst3!(SmokeSynth);

#[cfg(feature = "lv2")]
aura::export_lv2!(SmokeSynth);
