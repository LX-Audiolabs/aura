---
source: global
copied_by: template
date: 2026-08-10
adapted: false
reason: "AURA framework conventions for agent-assisted Rust audio plugin development"
id: aura
group: frameworks
summary: AURA framework — PluginLogic, #[derive(Params)] with explicit ids, cargo aura CLI, Slint+baseview UI stack, format wrappers.
triggers: aura, PluginLogic, derive(Params), cargo aura, aura-editor, aura-baseview, aura-build, aura-clap, aura-lv2, aura-vst3, aura-dsp
verify: Slint+baseview only; formats CLAP/VST3/LV2; derive(Params) with id=N; PluginLogic trait; cargo aura build|install|new|doctor
---

# AURA Framework

**Summary:** CLAP-first plugin framework for LX Audiolabs — PluginLogic trait,
`#[derive(Params)]` with explicit `id = N`, Slint + baseview UI, thin format
wrappers. Binary: `cargo aura`.

## Plugin structure

```rust
use aura::prelude::*;

#[derive(Params)]
struct MyParams {
    #[param(id = 0, name = "Gain", range = 0.0..=1.0)]
    gain: f32,
    #[param(id = 1, name = "Mix", range = 0.0..=100.0)]
    mix: f32,
}

struct MyPlugin;

impl PluginLogic for MyPlugin {
    fn process(&mut self, ctx: &mut ProcessCtx, params: &MyParams) {
        // audio callback — realtime-safe, no alloc
    }

    fn editor(&self) -> Option<Box<dyn Editor>> {
        Some(Box::new(AuraSlintEditor::<MyPlugin>::new()))
    }
}
```

## Derive rules (required)

| Rule | Why |
|------|-----|
| `#[derive(Params)]` on params struct | generates `<Struct>ParamId` enum + `id()`/`from_id()` |
| Every field needs `#[param(id = N, …)]` | wire-stable automation / state — AURA **requires** `id` |
| `id` must be unique within the struct | compile error if duplicate |
| Missing `id` → compile error | not silently auto-assigned (truce did that) |

## PluginLogic trait

| Method | Required | Notes |
|--------|----------|-------|
| `process()` | yes | `&mut self, ctx: &mut ProcessCtx, params: &P` — realtime-safe |
| `editor()` | no | return `AuraSlintEditor` for Slint UI; omit for headless |
| `activate()` / `deactivate()` | no | optional lifecycle hooks |

## cargo aura CLI

```bash
cargo aura new <name>          # scaffold plugin with derive(Params) + PluginLogic
cargo aura init <name>         # init in existing dir
cargo aura add <dep>           # add aura-* dependency
cargo aura build --clap -plug <name>   # build CLAP bundle
cargo aura install --clap -plug <name> # build + copy .clap to host dir
cargo aura doctor              # check toolchain + config
cargo aura preview             # preview .slint UI without compiling plugin

# Multi-plugin
cargo aura build --clap -plug aether meridian equilibrium
```

## UI stack (fixed)

- **Slint** only via `aura-baseview` + `aura-editor`
- `@aura` widgets: Knob, Slider, Toggle, Dropdown, Meter, XYPad
- Renderer backend: FemtoVG (OpenGL, default) / Skia / Software
- Shared UI: `lx-ui-slint` in product repo; portable widgets → `aura-build/ui/`

## Formats (thin wrappers)

| Format | Crate | Status |
|--------|-------|--------|
| CLAP | `aura-clap` | primary |
| VST3 | `aura-vst3` | supported |
| LV2 | `aura-lv2` | supported (no UI by design) |

All formats wrap the same `PluginLogic` trait — no format-shaped core.

## DSP

`aura-dsp` (juce_dsp-shaped) + `aura-midi` (messages / buffer).
Portable algorithms only — product infra (`lx-shm`, `lx-vault`) stays in plugin repo.

## Common mistakes

| Mistake | Fix |
|---------|-----|
| Missing `id = N` on `#[param(…)]` | add explicit, unique `id` |
| Using truce macros (`truce::plugin!`) in AURA plugin | remove; use `PluginLogic` trait + `cargo aura` |
| `use truce::*` in AURA workspace | replace with `use aura::prelude::*` |
| Manual `impl Params` instead of derive | use `#[derive(Params)]` (required for cutover) |
| No `PluginLogic` impl | add `impl PluginLogic for <Type> { fn process(…) { … } }` |

## Related loadouts

| Need | Skill |
|------|-------|
| Slint UI patterns | `04-ui/slint` |
| CLAP ship path | `03-formats/clap` |
| Realtime / thread safety | `00-core/dsp-realtime`, `00-core/audio-thread-boundary` |
| Versioning / release | `01-policy/versioning` |
