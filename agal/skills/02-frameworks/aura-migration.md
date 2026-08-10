---
source: global
copied_by: template
date: 2026-08-10
adapted: false
reason: "truce→AURA migration patterns for agal migration_legacy / aura_truce_migration findings"
id: aura-migration
group: frameworks
summary: truce→AURA migration — truce::plugin! → PluginLogic, params id migration, editor switch, cargo truce → cargo aura.
triggers: truce migration, truce to aura, migrate from truce, aura_truce_migration, migration_legacy, cutover, truce::plugin
verify: no truce imports remain; #[derive(Params)] with id=N; PluginLogic impl present; cargo aura build works
---

# truce → AURA migration

**Summary:** Step-by-step migration from truce-based plugins to AURA.
Covers macro replacement, params, editor, build, and common pitfalls.

## Migration checklist (ordered)

| Step | truce | AURA |
|------|-------|------|
| 1. Framework dep | `truce = { … }` | `aura = { path = "…" }` |
| 2. Plugin macro | `truce::plugin!(logic = T)` | `impl PluginLogic for T` + `aura::export!(T)` |
| 3. Params struct | `#[derive(Params)]` (auto `id`) | `#[derive(Params)]` **with explicit `id = N`** |
| 4. Editor | `truce_slint::TruceSlintEditor` / `lx_slint_editor` | `aura_editor::AuraSlintEditor` |
| 5. Imports | `use truce::prelude::*` | `use aura::prelude::*` |
| 6. Build | `cargo truce build --clap` | `cargo aura build --clap -plug <name>` |
| 7. Config | `truce.toml` | `aura.toml` |
| 8. Slint widgets | `@truce` | `@aura` |

## Step 1: Framework dependency

```toml
# Cargo.toml
[dependencies]
# was: truce = { path = "../../../truce" }
aura = { path = "../../../AURA/crates/aura" }   # or crates.io once published
```

Features stay format-gated: `clap`, `vst3`, `lv2`.

## Step 2: Plugin macro → PluginLogic trait

**Before (truce):**
```rust
use truce::prelude::*;

truce::plugin!(logic = MyPlugin);
```

**After (AURA):**
```rust
use aura::prelude::*;

struct MyPlugin;

impl PluginLogic for MyPlugin {
    type Params = MyParams;
    type DspState = MyDspState;

    fn info() -> PluginInfo { … }
    fn init(params: &Self::Params, sample_rate: f64) -> Self::DspState { … }
    fn process(state: &mut Self::DspState, params: &Self::Params, buffer: &mut AudioBuffer<'_, f32>, context: &mut ProcessContext) -> ProcessStatus { … }
    fn editor(params: Arc<Self::Params>) -> Option<Box<dyn Editor>> { … }
}

// Format export (one per format feature)
#[cfg(feature = "clap")]
aura::export!(MyPlugin);
#[cfg(feature = "vst3")]
aura::export_vst3!(MyPlugin);
#[cfg(feature = "lv2")]
aura::export_lv2!(MyPlugin);
```

`PluginLogic` replaces `truce::plugin!`. Format exports are explicit macros
(`aura::export!`, `aura::export_vst3!`, `aura::export_lv2!`), not implicit from features.

## Step 3: Params — add explicit IDs

**Before (truce — auto-assign):**
```rust
#[derive(Params)]
struct MyParams {
    #[param(name = "Gain", range = -24.0..=24.0, default = 0.0)]
    gain: FloatParam,
    #[param(name = "Mix", range = 0.0..=100.0, default = 50.0)]
    mix: FloatParam,
}
```

**After (AURA — `id = N` required):**
```rust
#[derive(Params)]
struct MyParams {
    #[param(id = 0, name = "Gain", range = -24.0..=24.0, default = 0.0)]
    gain: FloatParam,
    #[param(id = 1, name = "Mix", range = 0.0..=100.0, default = 50.0)]
    mix: FloatParam,
}
```

- `id = N` is **required** — missing → compile error.
- IDs must be **unique** within the struct.
- `#[derive(Params)]` generates `<Struct>ParamId` enum with `id()` and `from_id()`.
- Use `MyParamsParamId::Gain.id()` in editor/process, never hardcode.

## Step 4: Editor migration

**Before (truce):**
```rust
fn editor(…) -> Option<Box<dyn Editor>> {
    truce_slint::TruceSlintEditor::new(…).into_editor()
}
```

**After (AURA):**
```rust
fn editor(params: Arc<Self::Params>) -> Option<Box<dyn Editor>> {
    Some(
        aura_editor::AuraSlintEditor::new(
            (width, height),
            |ctx| { /* on_init: build UI, wire callbacks */ },
            |ui, ctx| { /* on_idle: sync params → UI */ },
        ).into_editor(),
    )
}
```

## Step 5: Import cleanup

```rust
// Remove all truce imports
// use truce::prelude::*;
// use truce_slint::TruceSlintEditor;
// use lx_slint_editor::LxSlintEditor;

// Replace with AURA prelude
use aura::prelude::*;
use aura_editor::AuraSlintEditor;
```

## Step 6: Build commands

```bash
# Before
cargo truce build --clap
cargo truce install --clap --release

# After
cargo aura build --clap -plug <name>
cargo aura install --clap -plug <name>
```

Multi-plugin: `cargo aura build --clap -plug aether meridian equilibrium`.

## Step 7: Config file

```bash
# Before
truce.toml          # [[plugin]] entries, features, install paths

# After
aura.toml           # [[plugin]] entries, features, install paths (same shape)
agal.toml           # agal orientation (optional)
```

`aura.toml` uses the same `[[plugin]]` TOML shape as truce — mostly rename + verify.

## Step 8: Slint widget imports

```slint
// Before
import { Knob, Slider } from "@truce";

// After — framework widgets
import { Knob, ParamSlider } from "@aura";

// Lx* branded widgets (unchanged if using lx-ui-slint)
import { LxKnob, LxSpectrum } from "../../../crates/lx-ui-slint/ui/lx.slint";
```

`@aura` replaces `@truce` for framework widgets. `Lx*` from `lx-ui-slint` stays the same
(that crate is agnostic to the framework).

## agal findings during migration

| Finding | Meaning | Action |
|---------|---------|--------|
| `migration_legacy` (error) | Plugin still on truce editor adapter | Switch to `aura-editor` (Step 4) |
| `aura_truce_migration` (info) | truce imports detected in AURA plugin | Clean up imports (Step 5) |
| `aura_missing_plugin_logic` (error) | No `impl PluginLogic` | Implement trait (Step 2) |
| `aura_missing_param_ids` (warn) | `#[param(…)]` without `id = N` | Add explicit IDs (Step 3) |

## Do not

- Mix truce and AURA imports in the same plugin
- Forget `aura::export!()` — without it the plugin won't be discovered by hosts
- Migrate `truce.toml` verbatim — verify `[[plugin]]` paths and feature flags
- Skip ID assignment — AURA derive won't compile without explicit `id = N`
- Assume `cargo truce install` paths match `cargo aura install` — check `aura.toml`

## See Also

- `02-frameworks/aura.md` — PluginLogic, derive(Params), cargo aura
- `04-ui/slint.md` — AuraSlintEditor, @aura widgets, param wiring
- `03-formats/clap.md` — CLAP ship path, validator
