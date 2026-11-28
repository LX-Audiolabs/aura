---
name: aura
description: AURA framework core knowledge and workflow — PluginLogic, Params derive, state codec, and the cargo-aura CLI. Use whenever working in the AURA codebase, writing or editing an AURA plugin, or answering questions about the AURA API surface.
---

# AURA Framework

AURA = **Audio Unified Rust Architecture** — CLAP-first plugin framework for LX
Audiolabs. Runtime + formats + build + CLI live in this repo.

## Canonical references (read before answering)

- `README.md` — scope, ship matrix, quick start, author surface.
- `crates/aura-core/src/plugin.rs` — the `PluginLogic` trait (source of truth).
- `crates/aura/src/lib.rs` — umbrella exports + `prelude`.
- `examples/` — compileable reference plugins (`smoke-gain`, `smoke-synth`, …).
- `docs/tooling.md` — dev tools + cadence. `docs/improvements.md` — backlog.
- `CLAP-GUIDE.md` — NOTE: may lag the current macro API (`#[id = "x"]` vs `#[param(id = 1)]`). Trust source + `examples/`, not prose.

The README "author surface" snippet has been known to drift from the real trait.
Always verify signatures against `crates/aura-core/src/plugin.rs`.

## Author surface (accurate as of 0.14.x)

```rust
use aura::prelude::*;

#[derive(Params)]
pub struct GainParams {
    #[param(id = 1, name = "Gain", range = "linear(-24, 24)", default = 0.0, unit = "db")]
    pub gain: FloatParam,
}

impl PluginLogic for MyGain {
    type Params = GainParams;
    type DspState = DspState;

    fn info() -> PluginInfo { /* clap_id, vendor, … */ }

    fn init(params: &Self::Params, sample_rate: f64) -> Self::DspState { todo!() }
    fn reset(state: &mut Self::DspState, params: &Self::Params, config: &AudioConfig) { /* */ }

    fn process(
        state: &mut Self::DspState,
        params: &Self::Params,
        buffer: &mut AudioBuffer<'_, f32>,
        context: &mut ProcessContext,
    ) -> ProcessStatus {
        // realtime-safe DSP
        ProcessStatus::Continue
    }
}

#[cfg(feature = "clap")]
aura::export!(MyGain);
```

Key trait methods (`PluginLogic`): `info`, `bus_layouts`, `init`, `reset`,
`process`, `editor`, `latency`, `tail_length`, `factory_presets`,
`state_context_supported`, `save_state_with_context`, `load_state_with_context`,
plus CLAP-only hooks (`note_names`, `remote_control_pages`, `on_param_mapping`,
`on_param_automation`, `tuning_changed`, `context_menu_*`).

## Params

- `#[derive(Params)]` on the params struct. Fields: `FloatParam`, `BoolParam`,
  `IntParam`, or `EnumParam` (`#[derive(ParamEnum)]`).
- `#[param(id = N, name = "...", ...)]` — **`id` is required and wire-stable**.
  Reordering fields must not renumber automation IDs.
- Attributes: `id`, `name`, `short_name`, `group`, `range`, `default`, `unit`,
  `flags`, `smooth`, `format`, `parse`, `chunk`.
- `flags` tokens: `automatable`, `hidden`, `readonly`, `bypass`, `modulatable`,
  `modulatable_per_note`.
- `#[persist]` / `#[persist("key")]` on extra fields → included in state blob.

## State

- Versioned envelope (v2): magic `AURA` + `u32 LE` version=2 + params payload +
  `#[persist]` payload. Codec: `aura::encode_state` / `aura::decode_state`.
- `StateContext::{Preset, Duplicate, Project}`. Default hooks share the AURA
  blob; custom load/save must `snap_smoothers()` after restore.

## Realtime contract (short form)

`process()` runs on the audio thread: no allocation, no locks, no file I/O, no
`println!`, no panic that can cross the C ABI. Deep review → load
`aura-dsp-review`.

## Exclusive backend features (gotcha)

`aura-baseview` / `aura-editor` require **exactly one** renderer backend
(`backend-femtovg-gl` default, `-wgpu`, `-skia`, `-software`). Tools that
enable all features (`cargo semver-checks`, `--all-features`) trip their
`compile_error!`. Use `--default-features` for semver-checks.

## cargo-aura CLI

`new`, `init`, `add`, `add-ui`, `build`, `install`, `watch`, `preview`, `run`,
`preset`, `doctor`. Build/install/watch gate on format flags
(`--clap`/`--vst3`/`--lv2`). Hot reload: `cargo aura watch --hot`.

## Versioning & release

Single workspace version in root `Cargo.toml`. Policy: `docs/versioning.md`;
changelog `CHANGELOG.md`; tags `vX.Y.Z`. MSRV Rust 1.95+.

## Which skill to load

| Question | Skill |
|----------|-------|
| `process()` realtime/numerics review | `aura-dsp-review` |
| CLAP extension / note-port / preset behavior | `aura-clap` |
| `.slint`, widgets, baseview, preview | `aura-ui` |
| Slint language itself | global `slint` |
| CLAP spec/threading model | global `clap-review` |
