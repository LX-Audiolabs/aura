---
id: slint
group: ui
summary: Slint UI patterns for AURA plugin editors — @aura widgets, AuraSlintEditor host-embed, callback wiring, build integration, shared Lx* components.
triggers: slint, editor UI, .slint, widget, plugin GUI, AuraSlintEditor, @aura, Knob, Meter, XYPad
verify: editor in AuraSlintEditor; param gestures via callbacks; @aura widgets from aura-build; slint_build::compile or aura_build materialize_assets
source: global
copied_by: template
date: 2026-08-10
adapted: true
reason: "AURA-specific Slint patterns: AuraSlintEditor host-embed, @aura widget catalog, aura-build pipeline"
---

# Slint UI (AURA stack)

**Summary:** AURA Slint patterns — `AuraSlintEditor` host-embed, `@aura` widget
catalog, callback→param wiring, `aura-build` compile pipeline, shared `Lx*`
components. Read this for any `.slint` work in AURA plugins.

## Architecture

```
.slint source                 Rust side
─────────────                 ─────────
ui/main.slint                 lib.rs
  export component AppWindow    slint::include_modules!();
  import { Knob } from "@aura"  impl PluginLogic {
    Knob {                        fn editor(…) -> AuraSlintEditor
      value <=> root.gain;           .on_init(|ctx| AppWindow::new())
      changed(v) => { … }            .on_idle(|ui, ctx| sync params → ui)
    }                              }
```

## AuraSlintEditor (host-embed)

`aura_editor::AuraSlintEditor` wraps a Slint component into a host-parented
window via `aura-baseview`. Always use this — never raw `slint::Window`.

```rust
fn editor(params: Arc<Self::Params>) -> Option<Box<dyn Editor>> {
    Some(
        aura_editor::AuraSlintEditor::new(
            (320, 220),                              // (width, height)
            |ctx| {                                   // on_init: build UI
                let ui = AppWindow::new().expect("slint component");
                let p = ctx.params.clone();
                ui.on_gain_changed(move |v| p.set_plain(P::Gain.id(), f64::from(v)));
                ui
            },
            |ui, ctx| {                               // on_idle: sync params → UI
                let v = ctx.params.get_plain(P::Gain.id()).unwrap_or(0.0) as f32;
                if (v - ui.get_gain()).abs() > 1.0e-4 {
                    ui.set_gain(v);
                }
            },
        )
        .into_editor(),
    )
}
```

**Rules:**
- `on_init` — build the Slint component, wire callbacks (param writes).
- `on_idle` — one-way sync params → UI. Guard with epsilon to avoid fighting drags.
- Use `<Params>ParamId` enum (`P::Gain.id()`) — never hardcode raw param indices.
- Return `.into_editor()` — wraps into `Box<dyn Editor>`.

## @aura widget catalog (`aura-build/ui/`)

Import from `@aura` — `aura-build` resolves the import path at compile time.

| Widget | File | Use |
|--------|------|-----|
| `Knob` | `knob.slint` | Rotary control: `value`, `label`, `minimum`, `maximum`, `value-text`, `changed(v)` |
| `ParamSlider` | `slider.slint` | Linear slider: same API as Knob |
| `Toggle` | `toggle.slint` | On/off switch: `checked`, `label`, `changed(checked)` |
| `Dropdown` | `dropdown.slint` | Combo select: `selected-index`, `model`, `changed(idx)` |
| `Meter` | `meter.slint` | Peak/RMS bar: `level` (0–1), `hold` |
| `XYPad` | `xy_pad.slint` | 2D pad: `x`, `y` (0–1), `changed(x, y)` |
| `AuraTheme` | `theme.slint` | M3 token palette: `surface`, `on-surface`, `radius-md`, `font-title`, … |

```slint
import { Knob, Toggle, Meter, AuraTheme } from "@aura";

export component AppWindow inherits Window {
    background: AuraTheme.surface;
    Knob {
        label: "Gain";
        minimum: -24.0; maximum: 24.0;
        value <=> root.gain;
        value-text: round(root.gain * 10) / 10 + " dB";
        changed(v) => { root.gain-changed(v); }
    }
}
```

## aura-build pipeline

Two paths — use the one that matches your crate type:

### Plugin crate (`cdylib`)
```rust
// build.rs
fn main() {
    aura_build::materialize_assets!();  // copies .slint → OUT_DIR, sets SLINT_LIBRARY_PATHS
}
```
```rust
// lib.rs
slint::include_modules!();  // compiles .slint → Rust types
```

### UI library crate (`lib` — e.g. `lx-ui-slint`)
```rust
// build.rs
fn main() {
    slint_build::compile("ui/lx.slint").unwrap();  // or aura_build for @aura deps
}
```

## Shared Lx* components (`lx-ui-slint`)

Product plugins use `lx-ui-slint` for LX-branded widgets beyond `@aura` basics:

```slint
import { Lx, LxKnob, LxShellHeader, LxSection, LxSpectrum } from "../../../crates/lx-ui-slint/ui/lx.slint";
```

AGAL auto-detects `uses_ui` edges when a plugin imports components that match an
`lx-ui-slint` export. Configure `ui_crates = ["lx-ui-slint"]` in `agal.toml`.

`@aura` = framework widgets (Knob, Meter, XYPad…). `Lx*` = product design system
(shell, spectrum, goniometer…). Both coexist — `@aura` does not aim to replace `Lx*`.

## Param wiring (two-way)

```slint
// .slint — declare callback + property
export component AppWindow {
    in-out property <float> gain;
    callback gain-changed(float);
    Knob {
        value <=> root.gain;              // two-way bind
        changed(v) => { root.gain-changed(v); }  // notify Rust
    }
}
```

```rust
// Rust on_init — wire callback → param write
let p = ctx.params.clone();
ui.on_gain_changed(move |v| p.set_plain(P::Gain.id(), f64::from(v)));

// Rust on_idle — readback: host/automation → UI
let v = ctx.params.get_plain(P::Gain.id()).unwrap_or(0.0) as f32;
if (v - ui.get_gain()).abs() > 1.0e-4 { ui.set_gain(v); }
```

**Never** read params or call `.set_plain` from `on_idle` without the epsilon guard
— per-frame sync fights the user's drag gesture.

## Do not

- Use raw `slint::Window` or `slint::ComponentHandle` for plugin editors
- Hardcode param indices — use `<Params>ParamId::<field>.id()`
- Call `params.set_plain()` from `on_idle` (one-way sync only)
- Put Slint component construction in `process()` — it's not realtime-safe
- Skip the epsilon guard in `on_idle` — causes UI jitter during host automation

## See Also

- `02-frameworks/aura.md` — PluginLogic, derive(Params)
- `02-frameworks/framework-patterns.md` — thread separation patterns
- `03-formats/clap.md` — CLAP GUI path (`clap.gui`)
