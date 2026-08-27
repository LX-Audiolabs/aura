---
source: global
copied_by: template
date: 2026-08-27
adapted: true
reason: "AURA PluginLogic surface: DspState, ProcessContext, thin format wrappers, LV2 UI"
id: aura
group: frameworks
summary: AURA framework — PluginLogic, #[derive(Params)] with explicit ids, cargo aura CLI, Slint+baseview UI stack, format wrappers.
triggers: aura, PluginLogic, derive(Params), cargo aura, aura-editor, aura-baseview, aura-build, aura-clap, aura-lv2, aura-vst3, aura-dsp
verify: Slint+baseview only; formats CLAP/VST3/LV2; derive(Params) with id=N; PluginLogic trait; cargo aura build|install|watch|mesh|new|doctor
---

# AURA Framework

**Summary:** CLAP-first plugin framework for LX Audiolabs — `PluginLogic` trait,
`#[derive(Params)]` with explicit `id = N`, Slint + baseview UI, thin format
wrappers. Binary: `cargo aura`.

## Plugin structure

```rust
use aura::prelude::*;

#[derive(Params)]
struct MyParams {
    #[param(id = 0, name = "Gain", range = 0.0..=1.0)]
    gain: f32,
}

struct MyPlugin;
struct DspState; // owned by the shell; allocate in init/reset, never in process

impl PluginLogic for MyPlugin {
    type Params = MyParams;
    type DspState = DspState;

    fn info() -> PluginInfo { /* clap_id / vst3_id / lv2_uri / category */ }

    fn init(_params: &MyParams, _sample_rate: f64) -> DspState {
        DspState
    }

    fn reset(_state: &mut DspState, _params: &MyParams, _config: &AudioConfig) {}

    fn process(
        _state: &mut DspState,
        _params: &MyParams,
        _buffer: &mut AudioBuffer<'_, f32>,
        _context: &mut ProcessContext,
    ) -> ProcessStatus {
        ProcessStatus::Continue
    }

    fn editor(params: Arc<MyParams>) -> Option<Box<dyn Editor>> {
        Some(
            AuraSlintEditor::new((320, 220), |_ctx| { /* build UI */ }, |_ui, _ctx| {})
                .into_editor(),
        )
    }
}
```

DSP state is **not** `self`. The shell owns `DspState`. `process` is realtime-safe:
no alloc, no lock, no `unwrap`.

## Derive rules (required)

| Rule | Why |
|------|-----|
| `#[derive(Params)]` on params struct | generates `<Struct>ParamId` enum + `id()`/`from_id()` |
| Every field needs `#[param(id = N, …)]` | wire-stable automation / state — AURA **requires** `id` |
| `id` must be unique within the struct | compile error if duplicate |
| Missing `id` → compile error | not silently auto-assigned |

## PluginLogic trait

| Method | Required | Notes |
|--------|----------|-------|
| `type Params` / `type DspState` | yes | `DspState: Send`; shell-owned |
| `info()` | yes | static ids (`clap_id`, `vst3_id`, `lv2_uri`) |
| `init()` / `reset()` | yes | allocate / clear DSP; `reset` on SR / block-size change |
| `process()` | yes | `state, params, buffer, &mut ProcessContext` → `ProcessStatus` |
| `editor()` | no | `Arc<Params>` → `AuraSlintEditor` / `None` headless |
| `bus_layouts()` | no | default stereo; override for mono / sidechain |
| `latency()` / `tail_length()` | no | samples; hosts read via CLAP/VST3 |
| `factory_presets()` / `load_preset_from_file()` | no | CLAP host-browser; default file load = v1 state blob |

`ProcessContext.ump` is native MIDI 2. `midi` is the 7-bit fallback. `notes` /
`notes_out` are CLAP-shaped (on/off/choke/expression/`NOTE_END`). VST3/LV2 must
not shrink this API — they down-convert.

## cargo aura CLI

```bash
cargo aura new <name>          # scaffold plugin with derive(Params) + PluginLogic
cargo aura init <name>         # init in existing dir
cargo aura add <name>          # plugins/<name>/ + aura.toml [[plugin]] + Cargo.toml members
cargo aura build --clap -plug <name>   # build CLAP bundle
cargo aura install --clap -plug <name> # build + copy .clap to host dir
cargo aura doctor              # check toolchain + config
cargo aura preview             # preview .slint UI without compiling plugin
cargo aura watch --clap --hot  # proxy .clap + replaceable .impl (re-add instance to swap DSP)
cargo aura mesh                # agal .  (orientation graph; optional)

# Multi-plugin
cargo aura build --clap -plug aether meridian equilibrium
```

## UI stack (fixed)

- **Slint** only via `aura-baseview` + `aura-editor`
- `@aura` widgets: Knob, Slider, Toggle, Dropdown, Meter, XYPad
- Renderer backend: FemtoVG (OpenGL, default) / Skia / Software
- Shared UI: `lx-ui-slint` in product repo; portable widgets → `aura-build/ui/`
- PeakMeter / FFT / Spectrum stay **product** design system — not `@aura`

## Formats (thin wrappers)

| Format | Crate | Status |
|--------|-------|--------|
| CLAP | `aura-clap` | primary |
| VST3 | `aura-vst3` | supported — same `PluginLogic`; UMP/notes down-converted |
| LV2 | `aura-lv2` | supported — TTL + UI via shared `Editor` when `editor()` is `Some` |

All formats wrap the same `PluginLogic` trait — no format-shaped core.
`aura::export!(MyPlugin)` (behind format features). Do **not** implement
`clap_plugin_factory` / VST3 `IComponent` / LV2 descriptors by hand.

## DSP

`aura-dsp` (juce_dsp-shaped) + `aura-midi` (messages / buffer).
Portable algorithms only — product infra (`lx-shm`, `lx-vault`) stays in plugin repo.

## Common mistakes

| Mistake | Fix |
|---------|-----|
| `fn process(&mut self, ctx: &mut ProcessCtx, …)` | real signature: `state, params, buffer, &mut ProcessContext` |
| Missing `id = N` on `#[param(…)]` | add explicit, unique `id` |
| Manual `impl Params` instead of derive | use `#[derive(Params)]` |
| Old-framework imports / `plugin!` macros | `use aura::prelude::*` + `impl PluginLogic` + `aura::export!` (see `aura-migration`) |
| Alloc / `Vec::push` in `process` | reserve in `init`/`reset`; wrappers cap events at 4096 |
| Raw `slint::Window` for a plugin GUI | `AuraSlintEditor` (see `04-ui/slint`) |

## Related loadouts

| Need | Skill |
|------|-------|
| Slint UI patterns | `04-ui/slint` |
| CLAP ship path | `03-formats/clap` |
| VST3 wrapper | `03-formats/vst3` |
| LV2 wrapper + TTL/UI | `03-formats/lv2` |
| Realtime / thread safety | `00-core/dsp-realtime`, `00-core/audio-thread-boundary` |
| Versioning / release | `01-policy/versioning` |
