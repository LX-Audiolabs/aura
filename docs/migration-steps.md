# AURA — strategy & build order

## Strategy (fixed)

**Reverse / product-led framework build** — not “fork truce then rename.”

```text
1) Ship AURA as a real framework
      installable & usable like truce:
      cargo aura new|build|install|doctor
      → build CLAP (then VST3/LV2) plugins with Slint+baseview

2) Prove it inside AURA
      in-tree examples/ smoke (and maybe one richer reference)
      cargo aura new skeleton shaped like our product plugins (ui/ + aura.toml + agal)

3) Only then migrate lx-audiolabs-plugins
      product catalog switches truce/lx-* → aura-*
      “as if we had always built them on AURA”
```

| We do | We do not (yet / ever) |
|-------|-------------------------|
| Finish **AURA basis** first | Bulk-migrate product plugins early |
| Path/git deps until basis usable | **crates.io only after** basis (`cargo aura` + CLAP + UI) |
| Learn from **how we built** aether/lucent/… | Copy every truce crate “because it exists” |
| truce + current plugins = **read-only design refs** | Vendor-dump truce into AURA |
| Targeted truce: Slint + CLAP/VST3/LV2 | egui/iced/AU/AAX zoo |

**Mental model:** truce is a general multi-UI/multi-format kit.  
**AURA** is the framework we *wish* we had after building LX plugins — same *workflow* as truce (`cargo … new/build/install`), narrower *surface*.

---

## “Basis fertig” — definition of done for phase Framework

AURA is ready for product cutover only when **all** of these work **without** the lx-audiolabs-plugins tree:

| # | Capability | Like truce | AURA target |
|---|------------|------------|-------------|
| 1 | Install toolchain | `cargo install cargo-truce` | **`cargo install cargo-aura`** (or path install) |
| 2 | New plugin | `cargo truce new` | **`cargo aura new <name>`** → layout like our plugins |
| 3 | Metadata | `truce.toml` | **`aura.toml`** (+ **`agal.toml`** in skeleton) |
| 4 | Params + DSP API | `PluginLogic` / params derive | **`aura-core` + `aura-params` (+ derive)** |
| 5 | UI | (various) | **`aura-editor` + `aura-build`** (`@aura`, FemtoVG default) |
| 6 | Build format | `--clap` etc. | **`cargo aura build --clap`** (then vst3/lv2) |
| 7 | Install into host path | `install --clap` | **`cargo aura install --clap`** (e.g. `%CLAPINS%`) |
| 8 | Sanity | validators / DAW load | **clap-validator** + Bitwig smoke on in-tree **example** |
| 9 | Docs | README | this file + root README scope |

Optional for v1 basis (nice, not gate): hot-reload shell, full MIDI 2.0, screenshots via slint-viewer.

**Out of scope until after cutover decision:** changing aether/meridian/… Cargo.toml to AURA.

---

## Naming (fixed)

| Old / interim | AURA |
|---------------|------|
| `cargo-truce` | **`cargo-aura`** → **`cargo aura …`** |
| `truce.toml` | **`aura.toml`** |
| `truce-*` / interim `lx-*` | **`aura-*`** |
| `use truce::*` | **`use aura::*`** |
| `lx-slint-baseview` | **`aura-baseview`** (window) + **`aura-editor`** (host adapter) |
| `lx-slint-build` | **`aura-build`** (`@aura`) |

---

## Stages (AURA-only until Basis fertig)

### Stage 0 — skeleton

- [x] LICENSE, compliance, README scope, workspace dirs
- [x] Naming: `aura-*`, `cargo aura`
- [x] Layout: `crates/` · `examples/` · `tools/`
- [ ] GitHub publish when useful
- [ ] Root `agal.toml` when workspace is agent-usable

### Stage 1 — core API

- [x] `aura-params`
- [x] `aura-core` (minimal `PluginLogic` / `Editor` / process)
- [x] Umbrella **`aura`** (`use aura::prelude::*`)
- [x] Grow core as formats need: ~~events~~ (param gesture queue → CLAP out_events), ~~transport~~ (CLAP → `ProcessContext.transport`), state, buses
  - buses/note-ports noch offen (note-ports = eigenes Todo, nicht Basis-kritisch)
- [ ] `aura-derive` (`Params`, plugin info) for author UX

### Stage 2 — CLAP path (first shippable format)

- [x] `aura-clap` (minimal: factory, stereo ports, params, process)
- [x] **Spec policy:** [free-audio/clap](https://github.com/free-audio/clap); bindings `clap-sys` (see `crates/aura-clap/README.md`)
- [x] Wire `aura` feature `clap` + `aura::export!`
- [x] In-tree **`examples/smoke-gain`** — clap-validator: 11 passed, 0 failed (state/GUI skipped)
- [x] `clap-validator` green (core path)
- [x] `clap.state` extension (save/load, flat LE blob) — validator state tests green
- [x] `clap.gui` extension (parented; host bridge: params + request_resize)
- [ ] Bump `clap-sys` when it tracks newer free-audio 1.2.x revisions (optional; 1.2 ABI ok)
- [ ] Bump `clap-sys` when it tracks newer free-audio 1.2.x revisions (optional; ABI ok)

### Stage 3 — UI complete for plugin authors

- [x] `aura-baseview` window stack (from lx-slint-baseview)
- [x] `aura-editor` thin re-export; host adapter next
- [x] `aura-build` (`@aura` + fonts)
- [x] Scaffold `ui/main.slint` uses `@aura` (`cargo aura new`)
- [x] **Host `Editor` adapter** in `aura-editor` (`AuraSlintEditor` impl `aura_core::Editor`)
- [x] smoke-gain GUI (Slint knob via `@aura`) — compiles + validator-stable; host open test: Bitwig pending
- [ ] Prefer Slint-native screenshot/viewer over truce-gui ports

### Stage 4 — toolchain (truce-workflow parity)

- [x] `tools/cargo-aura`: **`new`**, **`build`**, **`install`**, **`doctor`** (early; CLAP not valid yet)
- [x] `new` skeleton: Cargo.toml · aura.toml · agal.toml · ui/ · build.rs · src/lib.rs
- [x] Install: `cargo install --path tools/cargo-aura --force`
- [ ] Richer scaffold once PluginLogic + CLAP compile end-to-end
- [ ] `install --clap` copies `.clap` cleanly (smoke already validates)

### Stage 5 — VST3 / LV2

- [ ] After CLAP smoke is real
- [ ] Steinberg SDK checklist for VST3

### Stage 6 — polish (still inside AURA)

- [ ] Hot reload only if ROI clear
- [ ] MIDI 2.0 arch stubs
- [ ] Extra demos under `examples/` only if useful for cargo-aura docs
- [ ] **`aura-gui`** — Slint GUI for `cargo aura` commands (new, build, install, doctor) → graphical launcher for all CLI operations. Idea: see and click instead of terminal-only.

### Stage 7 — **product cutover** (only after Basis fertig)

- [ ] lx-audiolabs-plugins: truce / `lx-*` → **AURA path or git deps**
- [ ] `use aura::*`, `aura.toml`, editor/build switch
- [ ] CI matrix on product plugins
- [ ] Deprecate parallel truce path for LX products

---

## Explicit non-goals

- egui / iced / Vizia  
- AU / AAX / VST2  
- Full truce example zoo  
- Second UI path without Slint+baseview  
- Migrating product plugins before **`cargo aura install --clap`** works on an AURA-built smoke plugin  

---

## Reference inputs (read-only)

| Source | Why |
|--------|-----|
| **lx-audiolabs-plugins** | Layout, UX, what authors actually need |
| **truce-dev** | Proven param/process/format ideas to **selectively** port |
| **agal** | Orientation mesh in skeletons |

AURA is the **output** of that learning — a **gezielteres** truce, not a mirror.
