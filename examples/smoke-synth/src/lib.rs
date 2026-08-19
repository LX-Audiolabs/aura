//! Polyphonic smoke synth — proves `NoteVoiceTable` + per-note expressions /
//! `PARAM_MOD` + `NOTE_END`, plus MIDI fallback + `aura-dsp` (Oscillator + Adsr).
//!
//! 8 voices. Headless: hosts show their generic parameter UI.
//!
//! ```bash
//! cargo run -p cargo-aura -- install --clap --release -plug smoke-synth
//! ```

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::atomic::{AtomicU32, Ordering};

use aura::dsp::envelope::Adsr;
use aura::dsp::oscillator::{Oscillator, Waveform};
use aura::midi::midi_note_to_freq;
use aura::prelude::*;

static DEBUG_LEFT: AtomicU32 = AtomicU32::new(24);

fn debug_process(line: &str) {
    if DEBUG_LEFT.fetch_sub(1, Ordering::Relaxed) == 0 {
        return;
    }
    let Some(local) = std::env::var_os("LOCALAPPDATA") else {
        return;
    };
    let dir = std::path::PathBuf::from(local).join("AURA");
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("smoke-synth.log"))
    {
        let _ = writeln!(f, "{line}");
    }
}

const ATTACK: f32 = 0.005;
const DECAY: f32 = 0.05;
const SUSTAIN: f32 = 0.8;
const RELEASE: f32 = 0.1;
const MAX_VOICES: usize = 8;

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

pub struct SmokeSynth;

struct VoiceDsp {
    osc_sine: Oscillator,
    osc_saw: Oscillator,
    env: Adsr,
    gain_mod: f64,
    gain_plain: Option<f64>,
}

pub struct DspState {
    table: NoteVoiceTable,
    voices: Vec<VoiceDsp>,
    inbound: NoteBuffer,
    sr: f32,
    was_playing: bool,
}

fn make_osc(wave: Waveform, freq: f32, sample_rate: f32) -> Oscillator {
    Oscillator::new(wave, freq, sample_rate)
        .unwrap_or_else(|_| Oscillator::new(wave, 440.0, 44_100.0).unwrap())
}

fn make_env(sample_rate: f32) -> Adsr {
    Adsr::with_sample_rate(ATTACK, DECAY, SUSTAIN, RELEASE, sample_rate)
        .unwrap_or_else(|_| Adsr::new(ATTACK, DECAY, SUSTAIN, RELEASE).unwrap())
}

fn make_voice_dsp(sr: f32) -> VoiceDsp {
    VoiceDsp {
        osc_sine: make_osc(Waveform::Sine, 440.0, sr),
        osc_saw: make_osc(Waveform::Saw, 440.0, sr),
        env: make_env(sr),
        gain_mod: 0.0,
        gain_plain: None,
    }
}

impl PluginLogic for SmokeSynth {
    type Params = SynthParams;
    type DspState = DspState;

    fn info() -> PluginInfo {
        let mut info = PluginInfo::new(
            "AURA Smoke Synth 8v",
            "LX Audiolabs",
            env!("CARGO_PKG_VERSION"),
            "smoke-synth",
        );
        info.clap_id = "com.lx-audiolabs.aura.smoke-synth";
        info.vst3_id = "com.lx-audiolabs.aura.smoke-synth";
        info.lv2_uri = "https://lx-audiolabs.com/lv2/smoke-synth";
        info.category = PluginCategory::Instrument;
        info.accepts_midi_in = true;
        info.midi_input_dialect = aura::MidiDialect::Clap;
        info.voice_count = MAX_VOICES as u32;
        info.voice_capacity = MAX_VOICES as u32;
        info
    }

    fn bus_layouts() -> Vec<BusLayout> {
        vec![
            BusLayout::output_only(ChannelConfig::Stereo),
            BusLayout::output_only(ChannelConfig::Mono),
        ]
    }

    fn init(_params: &Self::Params, sample_rate: f64) -> Self::DspState {
        #[allow(clippy::cast_possible_truncation)]
        let sr = sample_rate as f32;
        let mut inbound = NoteBuffer::new();
        inbound.reserve(64);
        DspState {
            table: NoteVoiceTable::new(MAX_VOICES),
            voices: (0..MAX_VOICES).map(|_| make_voice_dsp(sr)).collect(),
            inbound,
            sr,
            was_playing: false,
        }
    }

    fn reset(state: &mut Self::DspState, _params: &Self::Params, config: &AudioConfig) {
        #[allow(clippy::cast_possible_truncation)]
        let sr = config.sample_rate as f32;
        state.sr = sr;
        for v in &mut state.voices {
            *v = make_voice_dsp(sr);
        }
        release_all(state, 0);
        state.was_playing = false;
    }

    fn process(
        state: &mut Self::DspState,
        params: &Self::Params,
        buffer: &mut AudioBuffer<'_, f32>,
        context: &mut ProcessContext,
    ) -> ProcessStatus {
        let playing = context.transport.is_some_and(|t| t.playing);
        if state.was_playing && !playing {
            release_all(state, 0);
        }
        state.was_playing = playing;

        if context.notes.is_empty() {
            state.inbound.clear();
            midi_to_notes(&context.midi, &mut state.inbound);
            let incoming = std::mem::take(&mut state.inbound);
            ingest(state, &incoming);
            state.inbound = incoming;
        } else {
            ingest(state, &context.notes);
        }

        if !context.notes.is_empty() || !context.midi.is_empty() {
            let mods: Vec<_> = context
                .notes
                .iter()
                .filter_map(|e| match e.kind {
                    NoteEventKind::ParamMod { param_id, amount } => Some(format!(
                        "id{param_id} n{} k{} {amount:.4}",
                        e.note_id, e.key
                    )),
                    NoteEventKind::ParamValue { param_id, plain } => Some(format!(
                        "val{param_id} n{} k{} {plain:.4}",
                        e.note_id, e.key
                    )),
                    _ => None,
                })
                .collect();
            let ons: Vec<_> = context
                .notes
                .iter()
                .filter(|e| matches!(e.kind, NoteEventKind::On { .. }))
                .map(|e| format!("n{}k{}", e.note_id, e.key))
                .collect();
            debug_process(&format!(
                "notes={} midi={} ons=[{}] mods=[{}] knob_mod={:.4} occ={}",
                context.notes.len(),
                context.midi.len(),
                ons.join(","),
                mods.join(","),
                params.gain.mod_amount(),
                state.table.occupied_count()
            ));
        }

        let n = buffer.num_samples();
        let outs = buffer.num_outputs();
        let knob = params.gain.raw_target();
        let sounding = state
            .voices
            .iter()
            .filter(|v| v.env.is_active())
            .count()
            .max(1);
        #[allow(clippy::cast_precision_loss)]
        let stack_norm = 1.0 / sounding as f32;
        for c in 0..outs {
            buffer.output(c).fill(0.0);
        }

        for i in 0..MAX_VOICES {
            let tv = state.table.voices()[i];
            let vd = &mut state.voices[i];
            if !tv.is_occupied() && !vd.env.is_active() {
                continue;
            }
            if tv.is_occupied() {
                retune(vd, tv.key, tv.tuning, state.sr);
            }
            let gain_plain = vd.gain_plain.unwrap_or(knob);
            // Bitwig Voice Stack often sends PARAM_MOD in 0..1. |amount|>1
            // is already dB (CLAP spec). Gain range is linear(-24, 24).
            let mod_db = if vd.gain_mod.abs() <= 1.0 {
                vd.gain_mod * 48.0
            } else {
                vd.gain_mod
            };
            #[allow(clippy::cast_possible_truncation)]
            let gain_db = (gain_plain + mod_db) as f32;
            let vel = if tv.is_occupied() { tv.velocity } else { 1.0 };
            let volume = if tv.is_occupied() { tv.volume } else { 1.0 };
            let pressure = if tv.is_occupied() { tv.pressure } else { 1.0 };
            let mix = if tv.is_occupied() {
                tv.brightness.clamp(0.0, 1.0)
            } else {
                0.0
            };
            let lin = 10.0f32.powf(gain_db / 20.0)
                * vel.clamp(0.0, 1.0)
                * volume.clamp(0.0, 4.0)
                * pressure.clamp(0.0, 1.0)
                * stack_norm;

            for s in 0..n {
                let sine = vd.osc_sine.next_sample();
                let saw = vd.osc_saw.next_sample();
                let sample = (sine * (1.0 - mix) + saw * mix) * vd.env.next_value() * lin;
                for c in 0..outs {
                    buffer.output(c)[s] += sample;
                }
            }
        }

        #[allow(clippy::cast_possible_truncation)]
        let end_at = n.saturating_sub(1) as u32;
        for i in 0..MAX_VOICES {
            if !state.voices[i].env.is_active() {
                state.table.mark_silent(i, end_at);
            }
        }
        state.table.flush_ends(&mut context.notes_out);
        ProcessStatus::Continue
    }
}

fn midi_to_notes(midi: &MidiBuffer, dest: &mut NoteBuffer) {
    for ev in midi.iter() {
        let msg = ev.message;
        if matches!(msg.status, aura::MidiStatus::ControlChange) && matches!(msg.data1, 120 | 123) {
            dest.push(NoteEvent {
                sample_offset: ev.sample_offset,
                note_id: -1,
                port_index: -1,
                channel: -1,
                key: -1,
                kind: NoteEventKind::Choke,
            });
            continue;
        }
        let Some(note) = msg.note_number() else {
            continue;
        };
        let key = i16::from(note);
        let note_id = i32::from(note);
        if msg.is_note_on() {
            dest.push(NoteEvent::on(
                ev.sample_offset,
                note_id,
                key,
                f64::from(msg.data2) / 127.0,
            ));
        } else if msg.is_note_off() {
            dest.push(NoteEvent::off(ev.sample_offset, note_id, key, 0.0));
        }
    }
}

fn find_slot(table: &NoteVoiceTable, ev: NoteEvent) -> Option<usize> {
    table
        .voices()
        .iter()
        .position(|v| v.is_occupied() && ev.matches_voice(v.note_id, v.key))
}

fn release_matching(state: &mut DspState, ev: NoteEvent) {
    for i in 0..MAX_VOICES {
        let tv = state.table.voices()[i];
        if tv.is_occupied() && ev.matches_voice(tv.note_id, tv.key) {
            state.voices[i].env.gate_off();
        }
    }
}

fn release_all(state: &mut DspState, sample_offset: u32) {
    for v in &mut state.voices {
        v.env.gate_off();
    }
    state.table.mark_all_silent(sample_offset);
}

fn ingest(state: &mut DspState, notes: &NoteBuffer) {
    for ev in notes.iter() {
        if matches!(ev.kind, NoteEventKind::Off { .. } | NoteEventKind::Choke) {
            release_matching(state, ev);
        }
    }
    state.table.apply(notes);
    // On first (alloc + reset), then mods. Bitwig Voice Stack often sends
    // PARAM_MOD in the same block *before* NOTE_ON; applying On last would
    // wipe gain_mod.
    for ev in notes.iter() {
        if let NoteEventKind::On { velocity } = ev.kind {
            if let Some(i) = find_slot(&state.table, ev) {
                let tv = state.table.voices()[i];
                start_voice(
                    &mut state.voices[i],
                    tv.key,
                    tv.tuning,
                    clap_unit(velocity),
                    state.sr,
                );
            }
        }
    }
    for ev in notes.iter() {
        match ev.kind {
            NoteEventKind::ParamMod { param_id, amount } => {
                if param_id == 1 {
                    for i in 0..MAX_VOICES {
                        let tv = state.table.voices()[i];
                        if tv.is_occupied() && ev.matches_voice(tv.note_id, tv.key) {
                            state.voices[i].gain_mod = amount;
                        }
                    }
                }
            }
            NoteEventKind::ParamValue { param_id, plain } => {
                if param_id == 1 {
                    for i in 0..MAX_VOICES {
                        let tv = state.table.voices()[i];
                        if tv.is_occupied() && ev.matches_voice(tv.note_id, tv.key) {
                            state.voices[i].gain_plain = Some(plain);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn clap_unit(v: f64) -> f32 {
    #[allow(clippy::cast_possible_truncation)]
    if v > 1.0 {
        (v / 127.0).clamp(0.0, 1.0) as f32
    } else {
        v.clamp(0.0, 1.0) as f32
    }
}

fn start_voice(v: &mut VoiceDsp, key: i16, tuning: f32, _velocity: f32, sr: f32) {
    retune(v, key, tuning, sr);
    v.env = make_env(sr);
    v.env.gate_on();
    v.gain_mod = 0.0;
    v.gain_plain = None;
}

fn retune(v: &mut VoiceDsp, key: i16, tuning_semis: f32, _sr: f32) {
    let key = u8::try_from(key.clamp(0, 127)).unwrap_or(69);
    let hz = midi_note_to_freq(key) * 2.0f32.powf(tuning_semis / 12.0);
    let _ = v.osc_sine.set_frequency(hz);
    let _ = v.osc_saw.set_frequency(hz);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev_on(id: i32, key: i16, vel: f64) -> NoteEvent {
        NoteEvent::on(0, id, key, vel)
    }

    fn ev_expr(id: i32, key: i16, kind: NoteExpression, value: f64) -> NoteEvent {
        NoteEvent {
            sample_offset: 1,
            note_id: id,
            port_index: 0,
            channel: 0,
            key,
            kind: NoteEventKind::Expression { id: kind, value },
        }
    }

    #[test]
    fn two_voices_keep_independent_tuning() {
        let params = SynthParams::default();
        let mut state = SmokeSynth::init(&params, 44_100.0);
        let mut notes = NoteBuffer::new();
        notes.push(ev_on(1, 60, 0.8));
        notes.push(ev_on(2, 64, 0.5));
        notes.push(ev_expr(1, 60, NoteExpression::Tuning, 12.0));
        ingest(&mut state, &notes);
        assert_eq!(state.table.occupied_count(), 2);
        assert!((state.table.voices()[0].tuning - 12.0).abs() < 1e-6);
        assert!(state.table.voices()[1].tuning.abs() < 1e-6);
        let expect = midi_note_to_freq(60) * 2.0;
        assert!((state.voices[0].osc_sine.frequency() - expect).abs() < 0.05);
        notes.clear();
        notes.push(NoteEvent {
            sample_offset: 2,
            note_id: 1,
            port_index: 0,
            channel: 0,
            key: 60,
            kind: NoteEventKind::ParamMod {
                param_id: 1,
                amount: 6.0,
            },
        });
        notes.push(NoteEvent {
            sample_offset: 2,
            note_id: 99,
            port_index: 0,
            channel: 0,
            key: 72,
            kind: NoteEventKind::ParamMod {
                param_id: 1,
                amount: -12.0,
            },
        });
        ingest(&mut state, &notes);
        assert!((state.voices[0].gain_mod - 6.0).abs() < 1e-12);
        assert!(state.voices[1].gain_mod.abs() < 1e-12);
    }

    #[test]
    fn note_end_after_release() {
        let params = SynthParams::default();
        let mut state = SmokeSynth::init(&params, 44_100.0);
        let mut notes = NoteBuffer::new();
        notes.push(ev_on(3, 60, 1.0));
        ingest(&mut state, &notes);
        notes.clear();
        notes.push(NoteEvent::off(0, 3, 60, 0.0));
        ingest(&mut state, &notes);
        assert!(state.table.voices()[0].is_occupied());
        assert!(!state.table.voices()[0].is_gated());
        while state.voices[0].env.is_active() {
            let _ = state.voices[0].env.next_value();
        }
        let mut out = NoteBuffer::new();
        state.table.mark_silent(0, 0);
        state.table.flush_ends(&mut out);
        assert!(matches!(out.as_slice()[0].kind, NoteEventKind::End));
        assert_eq!(out.as_slice()[0].note_id, 3);
    }

    #[test]
    fn wildcard_off_releases_all_stacked_copies() {
        let params = SynthParams::default();
        let mut state = SmokeSynth::init(&params, 44_100.0);
        let mut notes = NoteBuffer::new();
        notes.push(NoteEvent::on(0, 1, 60, 1.0));
        notes.push(NoteEvent::on(0, 2, 60, 1.0));
        ingest(&mut state, &notes);
        assert_eq!(state.table.occupied_count(), 2);
        notes.clear();
        notes.push(NoteEvent::off(0, -1, 60, 0.0));
        ingest(&mut state, &notes);
        assert!(!state.table.voices()[0].is_gated());
        assert!(!state.table.voices()[1].is_gated());
    }
}

#[cfg(feature = "clap")]
aura::export!(SmokeSynth);

#[cfg(feature = "vst3")]
aura::export_vst3!(SmokeSynth);

#[cfg(feature = "lv2")]
aura::export_lv2!(SmokeSynth);
