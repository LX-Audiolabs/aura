# AURA

[![License](https://img.shields.io/badge/license-GPL--3.0--or--later-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.92-orange.svg)](rust-toolchain.toml)
[![CI](https://github.com/LX-Audiolabs/aura/actions/workflows/framework.yml/badge.svg)](https://github.com/LX-Audiolabs/aura/actions/workflows/framework.yml)
[![Slint](https://img.shields.io/badge/UI-Slint-2379F4.svg)](https://slint.dev)
[![agal](https://img.shields.io/badge/powered%20by-agal-00ADD8.svg)](https://github.com/LX-Audiolabs/agal)

**Audio Unified Rust Architecture**  
*(DE-Gag: **AU**dio **RA**hmenwerk)*

CLAP-first plugin framework for **LX Audiolabs**.  
Runtime + formats + build + CLI live here. Agent orientation: **[agal](https://github.com/LX-Audiolabs/agal)**.

| | |
|--|--|
| **Status** | **0.9.x** — basis complete; used in production LX plugins ([lx-audiolabs-plugins](https://github.com/LX-Audiolabs/lx-audiolabs-plugins)) |
| **Dependency** | path / git deps today (`publish = false`); crates.io later |
| **License** | [GPL-3.0-or-later](./LICENSE) — see [docs/licensing-compliance.md](./docs/licensing-compliance.md) |
| **Rust** | 1.92+ MSRV (stable channel in `rust-toolchain.toml`), edition 2024 |

Commercial LX product plugins (aether, lucent, meridian, …) are **not** in this repository. The official public catalog is **[lx-audiolabs-plugins](https://github.com/LX-Audiolabs/lx-audiolabs-plugins)**. This repo is the **framework only** — smoke examples prove the ship path.

---

## Scope (hard)

AURA is **intentionally narrow**. If that is not your stack, use something else — no hard feelings.

| We commit to | We do **not** ship |
|--------------|--------------------|
| **UI:** [Slint](https://slint.dev) **only**, via **`aura-baseview`** (+ **`aura-editor`** host adapter) | egui, iced, Vizia as first-class UIs |
| **Formats:** **CLAP** primary; **VST3** / **LV2** only on the OS cells below | Audio Units (AU), AAX, VST2; “every format everywhere” |
| **Hosts:** Bitwig-first, then REAPER and other real CLAP/VST3/LV2 hosts | Pro Tools / Logic-only AU pipelines as a goal; Wine workarounds as product path |
| **Platforms:** Windows, Linux, macOS | iOS / embedded plugin hosts in v1 |

### Slint + baseview (always)

- **Always:** Slint UI + **baseview** host window (embed, scale, keys, clipboard, …).
- **Choose renderer** (features / project config), not toolkit:
  - **FemtoVG** (OpenGL) — default
  - **Skia** — optional
  - **Software / wgpu blit** — optional
- There is no “raw egui editor” mode and no second UI framework in AURA.

That stack is **`aura-baseview`** (window/renderer) + **`aura-editor`** (host adapter) + **`aura-build`** (compile-time). Every AURA plugin UI goes through it — not an optional addon.

### CLAP first (formats)

Same hardness as Slint: we pick a primary path and refuse a kitchen-sink matrix.

- **CLAP is the better plugin format** for what we build — modern, open, one native path on Linux, Windows, and macOS. That is the motto and the default (`cargo aura new`, CI validator, Bitwig-first host smoke).
- **VST3** is a **pragmatic second path** where the host world actually needs it: **Windows and macOS only**. Linux VST3 often means Wine or other heavy hacks — we do **not** treat that as a supported ship path.
- **LV2** is a **Linux-native** path (process/params/state/UI where the ecosystem fits). Not a Windows UI story; not a macOS story (`rust-lv2` / host reality).

**Ship matrix** (what CI installs / what we call a supported host path):

| Format | Linux | Windows | macOS | Role |
|--------|:-----:|:-------:|:-----:|------|
| **CLAP** | yes | yes | yes | Primary — always |
| **VST3** | — | yes | yes | Secondary — Win/mac hosts |
| **LV2** | yes | — | — | Secondary — Linux |

Wrappers may still compile on other OSes for unit tests; **product support** is the table above — just as “Slint only” does not mean “optional egui if you flip a feature.”

### Looking for egui, iced, AU, or every-format-everywhere?

AURA is **not** the right framework. Please use other open source projects that already optimize for that:

| Need | Go here |
|------|---------|
| Flexible Rust plugins, egui / iced / multiple UI styles, broad format matrix | **[nice-plug](https://codeberg.org/RustAudio/nice-plug)** (NIH-plug successor) |
| CLAP-centric Rust ecosystem / lower-level CLAP work | **[clack](https://github.com/prokopyl/clack)** (and related CLAP crates) |
| Full multi-format framework including AU/AAX paths, egui/iced/Vizia options | **[truce](https://github.com/truce-audio/truce)** · [truce.audio](https://truce.audio) |

We stand on the shoulders of that work (and permissive crates like CLAP bindings). AURA exists so **LX** can own a **Slint + baseview + CLAP-first** line — with thin VST3/LV2 where the OS and hosts justify them — without carrying every UI toolkit and every legacy format.

---

## Quick start

```bash
# 1) Clone + install the CLI (from this repo)
git clone https://github.com/LX-Audiolabs/aura.git
cd aura
cargo install --path tools/cargo-aura --locked

# 2) Point cargo-aura at the framework (path deps)
export AURA_PATH="$(pwd)"          # PowerShell: $env:AURA_PATH = (Get-Location).Path

# 3) Scaffold a plugin (CLAP always; add --vst3 / --lv2 if needed)
cargo aura new my-plugin
cd my-plugin

# 4) Build & install into the host search path
cargo aura install --clap --release

# 4b) Or stay in the loop: rebuild + reinstall on save
cargo aura watch --clap --hot

# 5) Optional: clap-validator on the installed .clap
clap-validator validate path/to/my-plugin.clap

# 6) Optional: regenerate the agal orientation mesh
cargo aura mesh
```

### In-repo smoke examples

Prove formats + UI without leaving this tree:

```bash
export AURA_PATH="$(pwd)"
cargo install --path tools/cargo-aura --locked

cargo aura install --clap --release -plug smoke-gain
# also: smoke-sidechain, smoke-midi-fx, smoke-synth
```

| Example | What it proves |
|---------|----------------|
| `examples/smoke-gain` | Stereo gain, Slint GUI, state, CLAP/VST3/LV2 |
| `examples/smoke-sidechain` | Main + one optional sidechain bus |
| `examples/smoke-midi-fx` | MIDI in/out (transpose thru) |
| `examples/smoke-synth` | Instrument / note path |

### UI preview (no DAW)

```bash
cargo aura preview
# or: cargo run -p aura-preview -- path/to/ui/main.slint
```

### Build / test the framework

```bash
cargo build --workspace
cargo test --workspace          # on non-Linux: --exclude aura-lv2
cargo clippy --workspace --all-targets
```

---

## Design principles

1. **Slint + baseview only** — renderer is a backend choice (FemtoVG / Skia / software); toolkit is not.
2. **CLAP first, thin formats** — one plugin logic API; VST3/LV2 only on the ship matrix; no format-shaped core.
3. **Framework layout** — `crates/` · `examples/` · `tools/` (product catalogs keep their own plugins outside AURA).
4. **One CLI:** **`cargo aura`** — `new`, `build`, `install`, `doctor`, `preview`, …
5. **KISS for humans and agents** — `aura.toml`, boring paths; orientation in **agal**.

---

## Workspace map

| Layer | Name |
|-------|------|
| Product name | **AURA** |
| Umbrella crate | **`aura`** → `use aura::prelude::*` |
| CLI package | **`cargo-aura`** → invoke as **`cargo aura …`** |
| Config | **`aura.toml`** |

| Crate / tool | Role |
|--------------|------|
| `aura` | Umbrella re-exports + features `clap` / `vst3` / `lv2` |
| `aura-core` | `PluginLogic`, process, buffer, state, host fence |
| `aura-params` + `aura-derive` | Params, smoothers, `#[derive(Params)]` with explicit `id = N` |
| `aura-clap` / `aura-vst3` / `aura-lv2` | Thin format wrappers |
| `aura-baseview` + `aura-editor` + `aura-build` | Slint window stack + host adapter + `@aura` widgets |
| `aura-dsp` + `aura-midi` | Portable DSP / MIDI helpers |
| `aura-shm` + `aura-hot` | Shared-memory IPC hub + CLAP hot-reload proxy (`cargo aura watch --hot`) |
| `aura-test` | State round-trip + process smokes (dev-dep) |
| `cargo-aura` | Scaffold, build, install, doctor, preview |
| `aura-preview` / `aura-gui` | Slint preview + optional project console |

---

## Author surface (minimal)

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

    fn info() -> PluginInfo { /* clap_id, vst3_id, … */ }

    fn process(
        _ctx: &mut PluginContext,
        params: &GainParams,
        _state: &mut DspState,
        buf: &mut AudioBuffer,
        _pc: &ProcessContext,
    ) -> ProcessStatus {
        // realtime-safe DSP here
        ProcessStatus::Continue
    }
}

#[cfg(feature = "clap")]
aura::export!(MyGain);
```

Param IDs are **required and wire-stable** — reordering fields does not renumber automation.

More detail: crate docs (`cargo doc -p aura --open`), [docs/versioning.md](./docs/versioning.md), [docs/dsp-layout.md](./docs/dsp-layout.md), [crates/aura-clap/README.md](./crates/aura-clap/README.md).

---

## Status & maturity

| Area | State |
|------|--------|
| `PluginLogic` + `#[derive(Params)]` | done |
| CLAP process / params / state / GUI | done (Bitwig host smoke) |
| Sample-accurate automation + mono mod | done |
| Sidechain (one optional bus) + MIDI I/O | done |
| Latency / remote-controls / tail / render | done |
| VST3 (Win/mac) | done (host smoke) |
| LV2 (Linux) process + UI extension | done (UI host smoke depends on host) |
| crates.io publish | **last** — after framework test pass; path/git only until then |
| `clap.preset-load` + factory discovery | done (`factory_presets`) |
| Poly mod / note expression | done (CLAP → `ProcessContext.notes`; `NoteVoiceTable` + `NOTE_END`) |
| Native MIDI 2 process | done (`ProcessContext.ump` / `ump_out`; 7-bit `midi` remains) |
| notes_out / arp-seq path | done (CLAP native; VST3/LV2 map On/Off/Choke) |

Changelog: [CHANGELOG.md](./CHANGELOG.md). Releases are tagged `vX.Y.Z`.

---

## Contributing

Issues and PRs welcome on this framework repo.

- Keep the **scope** (Slint + CLAP-first ship matrix). Do not add AU/egui/AAX “just in case.”
- Prefer small, tested changes. One assert for non-trivial logic.
- Match workspace versioning: single version in root `Cargo.toml` + `CHANGELOG.md`.
- CI: Framework (build/install smokes per OS matrix) + Quality (fmt, clippy, tests).

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets
```

---

## Acknowledgments

`aura-dsp` benefited from several open-source DSP projects whose code, concepts, and signal-flow models directly shaped our designs:

| Project | Link | What we learned |
|---------|------|-----------------|
| **naad** (rust-old) | [MacCracken/naad](https://github.com/MacCracken/naad/tree/main/rust-old) | Node-based DSP graph, real-time safe wiring, Faust-like operator composition |
| **fundsp** | [SamiPerttu/fundsp](https://github.com/SamiPerttu/fundsp) | Composable signal graphs, SIMD-friendly `AudioUnit` traits, declarative DSP in Rust |
| **infinitedsp** | [infinitedsp](https://github.com/infinitedsp/infinitedsp) | Type-level DSP, compile-time graph verification, zero-cost abstraction patterns |

Thanks to the maintainers of these projects — your work directly influenced our DSP layer.

Also: [truce](https://github.com/truce-audio/truce), [CLAP](https://github.com/free-audio/clap), [Slint](https://slint.dev), and the Rust audio community.

---

## License

Copyright © 2026 LX Audiolabs

This project is free software under the **GNU General Public License v3.0 or later**.  
Distributing plugins that link AURA implies GPL obligations for that combined work. Selling with source is fine; closed-only ships are not the goal.

Third-party notes (including **Slint** triple license — default ship path **GPLv3**):  
[docs/licensing-compliance.md](./docs/licensing-compliance.md).
