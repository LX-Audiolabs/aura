---
name: aura-author
description: Writing or editing a plugin that links AURA (lx-aura) — PluginLogic, Params derive, state hooks, export!, and the cargo-aura workflow. Use when working in a plugin repo (lx-audiolabs-plugins, Helmira, Mensor, …) that depends on AURA, NOT when editing the AURA framework itself.
---

# Writing an AURA plugin (author view)

AURA = **Audio Unified Rust Architecture** — CLAP-first plugin framework.
You link the umbrella crate `lx-aura` (Rust crate name `aura`) and implement
one trait. You do **not** need to know the format wrappers' internals.

## Canonical references

- API docs (canonical): <https://docs.rs/lx-aura> · <https://docs.rs/lx-aura-core> · <https://docs.rs/lx-aura-params>
- Local framework source (for reading real signatures): your AURA checkout —
  the `path` in `lx-aura = { path = "…" }` in your `Cargo.toml`, or the
  `AURA_PATH` env var. Compileable examples live in `<aura-checkout>/examples/`
  (`smoke-gain`, `smoke-synth`, …).
- The README "author surface" snippet can lag the real trait. Trust `examples/`
  and docs.rs over prose.

## Minimal plugin

```rust
use aura::prelude::*;

#[derive(Params)]
pub struct GainParams {
    #[param(id = 1, name = "Gain", range = "linear(-24, 24)", default = 0.0, unit = "db")]
    pub gain: FloatParam,
}

pub struct MyGain;
pub struct DspState;

impl PluginLogic for MyGain {
    type Params = GainParams;
    type DspState = DspState;

    fn info() -> PluginInfo {
        PluginInfo::new("My Gain", "LX Audiolabs", "0.1.0", "com.lx.my-gain")
        // then: category, clap_id, vst3_id, accepts_midi_in, voice_*, supports_tuning, …
    }

    fn init(_params: &Self::Params, _sample_rate: f64) -> Self::DspState { DspState }

    fn reset(_state: &mut Self::DspState, _params: &Self::Params, _config: &AudioConfig) {}

    fn process(
        _state: &mut Self::DspState,
        params: &Self::Params,
        buffer: &mut AudioBuffer<'_, f32>,
        _context: &mut ProcessContext,
    ) -> ProcessStatus {
        // realtime-safe DSP; read smoothed param values, write buffer channels
        ProcessStatus::Continue
    }
}

#[cfg(feature = "clap")]
aura::export!(MyGain);
```

The `process` signature is exactly
`process(state, params, buffer, context) -> ProcessStatus` with
`context: &mut ProcessContext`.

## PluginInfo (what authors set)

`PluginInfo::new(name, vendor, version, bundle_id)` then fill: `category`
(`Effect`/`Instrument`/`Analyzer`/`NoteEffect`), `clap_id`, `vst3_id`, `lv2_uri`,
`accepts_midi_in`, `emits_midi`, `midi_input_dialect` / `midi_output_dialect`
(`Midi1`/`Midi2`/`Clap`), `voice_count` / `voice_capacity` (>0 to advertise
voice-info + Bitwig Voice Stack), `supports_tuning`.

## Params

- `#[derive(Params)]` on the params struct. Types: `FloatParam`, `BoolParam`,
  `IntParam`, `EnumParam` (`#[derive(ParamEnum)]`).
- `#[param(id = N, …)]` — **`id` required and wire-stable**. Never renumber.
- Attributes: `id`, `name`, `short_name`, `group`, `range`, `default`, `unit`,
  `flags`, `smooth`, `format`, `parse`, `chunk`.
- `flags`: `automatable`, `hidden`, `readonly`, `bypass`, `modulatable`,
  `modulatable_per_note`.
- `#[persist]` / `#[persist("key")]` on extra fields → persisted in state.

## State

- Default host save/load is handled for you (versioned v2 envelope). Override
  only if you need a custom format:
  `PluginLogic::save_state_with_context` / `load_state_with_context`, with
  `StateContext::{Preset, Duplicate, Project}`.
- Custom load must call `params.snap_smoothers()` after restore, or you get
  audible ramps.

## Realtime contract (one line)

`process()` runs on the audio thread: **no allocation, no locks, no I/O, no
`println!`, no panics that unwind across the C ABI.** Pre-allocate in
`init`/`reset`. Deep review → `aura-dsp-review` skill.

## Note handling (instruments / note FX)

- Read host notes from `ProcessContext.notes` (sorted by sample offset);
  7-bit mirror on `midi`, MIDI 2 native on `ump`.
- Track voices with `NoteVoiceTable::new(max_voices)` (clamped 1..=128) —
  `apply(&notes)` + `flush_ends()` are allocation-free after construction.
- Emit generated notes on `ProcessContext.notes_out`; set
  `PluginInfo.emits_midi = true` so a note output port exists.

## UI (if any)

Slint only, via `lx-aura-baseview` + `lx-aura-editor`. Pick **exactly one**
renderer backend feature (`backend-femtovg-gl` default, `-wgpu`, `-skia`,
`-software`). UI reads snapshots only — never write DSP state from the editor.
Details → `aura-ui` skill.

## cargo-aura workflow

```bash
cargo aura new <name> [--vst3] [--lv2] [--kind effect|effect-mono|analyzer|instrument]
cargo aura add <name>                 # another plugin under plugins/<name>/
cargo aura build   --clap --release
cargo aura install --clap --release
cargo aura watch   --clap --hot        # hot reload
cargo aura preview                     # UI without a DAW
cargo aura doctor                      # toolchain / AURA_PATH / clap-validator
```

Set `AURA_PATH` to the framework checkout so `cargo aura` resolves path deps.

## Deeper topics → load

| Topic | Skill |
|-------|-------|
| `process()` realtime/numerics review | `aura-dsp-review` |
| CLAP extension / note-port / preset behavior | `aura-clap` |
| `.slint`, widgets, baseview, preview | `aura-ui` |
| Slint language itself | global `slint` |
| CLAP spec / threading model | global `clap-review` |

The `aura-*` review/wrapper skills are project-local to the AURA repo — load
them when you need framework-internals detail.
