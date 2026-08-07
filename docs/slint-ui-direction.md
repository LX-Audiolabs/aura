# Slint UI direction — references, aura-gui, visual identity

Living note: upstream Slint resources we can learn from, and **how AURA should look**
(plugin editors + future `aura-gui`). Not an implementation plan — decisions + constraints.

Last pass: 2026-08-07 (U1/U2: `@aura` + smoke-gain restyled to M3 dark tokens).

Related: [migration-steps.md](./migration-steps.md) (Stage 6 aura-gui),  
[licensing-compliance.md](./licensing-compliance.md) (Slint triple-license),  
`crates/aura-build` (`@aura` widgets).

---

## Why this note

1. Stage 6 envisions a **visual AURA tool** (`aura-gui`) for init / new / build / install — CLI remains source of truth.
2. Current **example / scaffold UI** (smoke-gain + `@aura` knobs) still reads **truce/egui-adjacent** (dark panel, blue rotary). That works technically, but is **not** the visual identity we want for “AURA-native.”
3. Upstream Slint now ships a real **Material Design 3** library and long-running reference apps/templates we can study.

---

## Upstream references (analyse)

### 1. [slint-rust-template](https://github.com/slint-ui/slint-rust-template)

| | |
|--|--|
| **What** | Minimal Rust + Slint **desktop app** starter (also used with cargo-generate / zip) |
| **Shape** | `ui/*.slint` · `build.rs` → `slint_build::compile` · `slint::include_modules!()` · `std-widgets` |
| **License / deps** | App template; pins `slint` / `slint-build` (same major as ecosystem) |

**Take for AURA**

| Useful | Not useful |
|--------|------------|
| Canonical **standalone** layout for a tool binary (`aura-gui`, `aura-preview` already similar) | **Not** a plugin/cdylib template — no host embed, no `PluginLogic` |
| Callback + property wiring demo for authors | Do not replace `cargo aura new` with this template |
| IDE/LSP tips for `.slint` | std-widgets look is generic Fluent, not our product look |

**Verdict:** reference for **tool UIs** and “how Slint documents Rust interop.” Scaffold stays AURA-shaped (`aura.toml`, derive, formats).

---

### 2. [cargo-ui](https://github.com/slint-ui/cargo-ui)

| | |
|--|--|
| **What** | Full Slint **GUI for Cargo** (`cargo ui`): packages, features, dep tree, crates.io install, errors |
| **Architecture** | Slint window + **worker threads** + **message channel** (`CargoMessage`); callbacks only enqueue work; workers push model updates back |
| **UI split** | `ui/main.slint`, `cargo.slint`, `install.slint`, `rustup.slint`, … |
| **License** | Source MIT OR Apache-2.0; **binary GPLv3** because of GPL Slint deps — same shape as our stack |

**Take for AURA (`aura-gui`)**

| Pattern | AURA mapping |
|---------|----------------|
| GUI is a **thin shell** over existing operations | Buttons call the **same** paths as `cargo aura init/new/build/install/doctor` (flags 1:1) |
| Long work off UI thread | Build/install/validator must not freeze the window |
| File open / folder pick (`rfd`) | “Open project”, “pick install path” |
| Multi-tab console | Init wizard · day-to-day project · doctor status |
| Do **not** vendor cargo-ui | Inspiration only; domain is AURA, not general cargo |

**Verdict:** best **architecture reference** for Stage 6 aura-gui. Already aligned with migration-steps: *“GUI is sugar; CLI remains source of truth.”*

---

### 3. [Material Design 3 for Slint](https://github.com/slint-ui/slint/tree/master/ui-libraries/material)

| | |
|--|--|
| **What** | Official **Material 3** component set for Slint (gallery, docs, templates) |
| **Ship** | [material.slint.dev](https://material.slint.dev/) · zip / `@material` library path · [material-rust-template](https://github.com/slint-ui/material-rust-template) |
| **Component license** | **MIT** on the `.slint` library sources (`ui-libraries/material/src/LICENSE.md`) — easy to vendor or path-dep under our GPL plugins |
| **Docs** | [Getting started](https://material.slint.dev/getting-started/) — same `CompilerConfiguration::with_library_paths` pattern as our `@aura` |

**What it actually is**

App / mobile / desktop **chrome and forms**, not a DAW control set:

- Buttons (filled / tonal / outline / text / FAB), icon buttons  
- TextField, Switch, Checkbox, Radio, Slider, Chips  
- Nav (rail, drawer, bar), AppBar, Tabs, Dialogs, Cards, Lists, Snackbar  
- **Theming tokens:** `MaterialPalette`, `MaterialScheme(s)`, typography, metrics, animations  
- **No** rotary knob, **no** peak/RMS meter, **no** XY pad, **no** stepped “param value + unit” widget

Wiring is analogous to AURA:

```rust
// conceptual — same idea as aura_build materialize_assets
library_paths: "material" → path/to/material.slint
// .slint: import { FilledButton, Slider } from "@material";
```

**Verdict for “basis UI”:** **yes as visual language and for tool UIs; no as sole widget kit for plugins.**

---

## Visual identity decision (proposed)

### Problem

`examples/smoke-gain` + `@aura` knobs:

- Dark slab `#1a1a1e`, blue fill `#4d99f2`, classic rotary — **reads like truce/egui plugin kits**
- Fine as **host-smoke proof**; wrong as **brand default** for AURA authors

### Direction

```text
                    ┌─────────────────────────────────────┐
  aura-gui / tools  │  Material 3 (@material) for chrome  │
                    │  forms, nav, dialogs, progress        │
                    └─────────────────────────────────────┘
                                      │ tokens / palette optional share
                    ┌─────────────────▼───────────────────┐
  plugin editors    │  @aura = audio-first widgets         │
                    │  Knob · Meter · ParamSlider · …      │
                    │  restyled: M3-aligned tokens, not    │
                    │  truce/egui chrome clone              │
                    └─────────────────────────────────────┘
```

| Layer | Default look | Widget source |
|-------|--------------|---------------|
| **Plugin editor** (smoke, scaffold, products) | AURA-native (not truce-lookalike) | **`@aura`** (keep audio widgets; **retheme**) |
| **aura-gui / init wizard** | Material 3 desktop | **`@material`** (+ thin AURA chrome if needed) |
| **aura-preview** | whatever the author `.slint` uses | no forced skin |

### Material for plugins — when

| Use Material in a plugin UI when… | Keep / extend `@aura` when… |
|-----------------------------------|-----------------------------|
| Settings panels, toggles, text, dialogs | Continuous params (knobs), meters, XY, multi-unit param display |
| About / license / rescan chrome | Anything that must match DAW automation density |
| Optional “modern form” sections | Default scaffold should still look like an **audio** editor |

### Explicit non-goals

- Do **not** make every plugin a Material mobile mockup.  
- Do **not** drop knobs “because Material has Slider.”  
- Do **not** vendor-dump truce UI crates “for familiarity.”  
- Do **not** block basis (formats / process) on a full visual redesign — redesign is **author UX / Stage 6**, not CLAP DoD.

---

## Concrete follow-ups (not started)

Priority is **after** Stage 5 format work unless we want a quick visual win on smoke-gain only.

| ID | Item | Notes |
|----|------|--------|
| **U1** | ~~Restyle `@aura` tokens~~ | **done** — `AuraTheme` (M3 dark) + restyled Knob/Meter/… |
| **U2** | ~~Reskin smoke-gain + scaffold~~ | **done** — surface card + `AuraTheme` |
| **U3** | Spike: path-dep or vendored `@material` for a **future** `tools/aura-gui` | Study cargo-ui worker pattern |
| **U4** | Document `@aura` vs `@material` in scaffold README / `cargo aura new` comments | Partial via scaffold header comment |
| **U5** | Optional: hybrid smoke — Material Switch + `@aura` Knob side by side | later / aura-gui |
| **U6** | Re-smoke Bitwig after restyle | Confirm parented editor still opens cleanly |

---

## License snapshot (UI libs)

| Piece | License note |
|-------|----------------|
| Slint runtime (in plugin binary) | Our default: **GPLv3** option — see [licensing-compliance.md](./licensing-compliance.md) |
| Material **component `.slint` sources** | **MIT** (SixtyFPS) — fine to ship / adapt under GPL plugins with notice |
| cargo-ui | Study only; don’t copy wholesale into AURA |
| Our `@aura` widgets | AURA / GPL-3.0-or-later workspace |

---

## One-line summary

**Material 3 = yes for visual language and aura-gui; audio controls stay `@aura`, restyled so examples stop looking like truce/egui. cargo-ui = architecture model for the tool shell; slint-rust-template = standalone app shape only.**
