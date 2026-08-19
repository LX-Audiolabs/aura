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

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::atomic::{AtomicU32, Ordering};

use aura::prelude::*;

static DEBUG_LEFT: AtomicU32 = AtomicU32::new(24);

fn debug_process(line: &str) {
    if DEBUG_LEFT.load(Ordering::Relaxed) == 0 {
        return;
    }
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
        .open(dir.join("smoke-midi-fx.log"))
    {
        let _ = writeln!(f, "{line}");
    }
}

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
            "AURA Smoke MIDI FX +",
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
        // Bitwig Note FX forwards the MIDI dialect more reliably than CLAP notes.
        info.midi_input_dialect = aura::MidiDialect::Midi1;
        info.midi_output_dialect = aura::MidiDialect::Midi1;
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

        let n = buffer.num_samples();
        let ch = buffer.num_outputs().min(buffer.num_inputs());
        for c in 0..ch {
            for i in 0..n {
                let s = buffer.input(c)[i];
                buffer.output(c)[i] = s;
            }
        }
        for c in ch..buffer.num_outputs() {
            buffer.output(c).fill(0.0);
        }

        // One dialect only. Emitting CLAP notes *and* MIDI made Bitwig
        // spawn a second voice that never got a matching off (stuck tail).
        if context.notes.is_empty() {
            for ev in context.midi.iter() {
                let msg = ev.message;
                let out_msg = if msg.is_note_on() || msg.is_note_off() {
                    let note = msg
                        .note_number()
                        .map(|note| {
                            u8::try_from((i32::from(note) + transpose).clamp(0, 127)).unwrap_or(0)
                        })
                        .unwrap_or(msg.data1);
                    MidiMessage::raw(msg.status_byte(), note, msg.data2)
                } else {
                    msg
                };
                context.midi_out.push(ev.sample_offset, out_msg);
            }
        } else {
            for ev in context.notes.iter() {
                let mut out = ev;
                if out.key >= 0 {
                    out.key =
                        i16::try_from((i32::from(out.key) + transpose).clamp(0, 127)).unwrap_or(0);
                }
                context.notes_out.push(out);
            }
        }

        debug_process(&format!(
            "frames={} in_n={} in_m={} out_n={} out_m={} audio_in={} audio_out={} xpose={}",
            n,
            context.notes.len(),
            context.midi.len(),
            context.notes_out.len(),
            context.midi_out.len(),
            buffer.num_inputs(),
            buffer.num_outputs(),
            transpose
        ));

        ProcessStatus::Continue
    }
}

#[cfg(feature = "clap")]
aura::export!(SmokeMidiFx);

#[cfg(feature = "vst3")]
aura::export_vst3!(SmokeMidiFx);

#[cfg(feature = "lv2")]
aura::export_lv2!(SmokeMidiFx);
