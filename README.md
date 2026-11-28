# AURA

[![License](https://img.shields.io/badge/license-GPL--3.0--or--later-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.95-orange.svg)](rust-toolchain.toml)
[![Slint](https://img.shields.io/badge/UI-Slint-2379F4.svg)](https://slint.dev)

**Audio Unified Rust Architecture**\
*(DE-Gag: **AU**dio **RA**hmenwerk)*

CLAP-first plugin framework for **LX Audiolabs**.\
Runtime + formats + build + CLI live here.

Published API baseline: **0.15.0** ([crates.io](https://crates.io/crates/lx-aura/0.15.0)).
The versioned [API reference on docs.rs](https://docs.rs/lx-aura/0.15.0/aura/) is
canonical; the Rust example below targets that release and was also checked
against the **0.15.0** release source on **2026-11-28**. Repository quickstart and smoke
examples use the checkout, including release changes and any subsequent `[Unreleased]` work in
[CHANGELOG.md](CHANGELOG.md).

| | |
|--|--|
| **Status** | **0.15.0 released** — see migration notes in [CHANGELOG.md](CHANGELOG.md); framework used in production LX plugins ([lx-audiolabs-plugins](https://github.com/LX-Audiolabs/lx-audiolabs-plugins)) |
| **Dependency** | `lx-aura` / `lx-aura-*` published to crates.io; `aura-host` / `aura-hot` / examples path-only (`publish = false`) |
| **License** | [GPL-3.0-or-later](./LICENSE) |
| **Rust** | 1.95+ MSRV (stable channel in `rust-toolchain.toml`), edition 2024 |

Commercial LX product plugins (aether, lucent, meridian, …) are **not** in this repository. The official public catalog is **[lx-audiolabs-plugins](https://github.com/LX-Audiolabs/lx-audiolabs-plugins)**. This repo is the **framework only** — smoke examples prove the ship path.

---

## Scope (hard)

AURA is intentionally narrow. Wrong stack → use another framework.

| We commit to | We do **not** ship |
|--------------|--------------------|
| **UI:** [Slint](https://slint.dev) **only**, via **`aura-baseview`** (+ **`aura-editor`** host adapter) | egui, iced, Vizia as first-class UIs |
| **Formats:** **CLAP** primary; **VST3** / **LV2** only on the OS cells below | Audio Units (AU), AAX, VST2; “every format everywhere” |
| **Hosts:** Bitwig-first, then REAPER and other real CLAP/VST3/LV2 hosts | Pro Tools / Logic-only AU pipelines as a goal; Wine workarounds as product path |
| **Platforms:** Windows, Linux, macOS | iOS / embedded plugin hosts in v1 |

### Slint + baseview (always)

- **Always:** Slint UI + **baseview** host window (embed, scale, keys, clipboard, …).
- **Choose renderer** (features / project config), not toolkit:
  - **`FemtoVG`** — default: `backend-femtovg-gl` (OpenGL) or `backend-femtovg-wgpu` (wgpu)
  - **Skia** — optional (`backend-skia`)
  - **Software** — optional (`backend-software`)
- There is no “raw egui editor” mode and no second UI framework in AURA.

Required UI path: **`aura-baseview`** (window/renderer) + **`aura-editor`** (host adapter) + **`aura-build`** (compile-time).

### CLAP first (formats)

Same rule as Slint: one primary format, no kitchen-sink matrix.

- **CLAP** primary — one native path on Linux, Windows, macOS (`cargo aura new`, local validator, Bitwig-first).
- **VST3** second path: **Windows and macOS only**. Linux VST3 (Wine etc.) unsupported.
- **LV2** = Linux only (process/params/state/UI). No Win/mac ship path.

**Ship matrix** (what CI installs / what we call a supported host path):

| Format | Linux | Windows | macOS | Role |
|--------|:-----:|:-------:|:-----:|------|
| **CLAP** | yes | yes | yes | Primary — always |
| **VST3** | — | yes | yes | Secondary — Win/mac hosts |
| **LV2** | yes | — | — | Secondary — Linux |

Wrappers may still compile on other OSes for unit tests. **Product support** = the table above.

### Looking for egui, iced, AU, or every-format-everywhere?

Use another project:

| Need | Go here |
|------|---------|
| Flexible Rust plugins, egui / iced / multiple UI styles, broad format matrix | **[nice-plug](https://codeberg.org/RustAudio/nice-plug)** (NIH-plug successor) |
| CLAP-centric Rust ecosystem / lower-level CLAP work | **[clack](https://github.com/prokopyl/clack)** (and related CLAP crates) |
| Full multi-format framework including AU/AAX paths, egui/iced/Vizia options | **[truce](https://github.com/truce-audio/truce)** · [truce.audio](https://truce.audio) |

AURA = **Slint + baseview + CLAP-first**, with thin VST3/LV2 only on the ship matrix. Credit: clack, nice-plug, truce, clap bindings.

---

## CLAP Guide

AURA implements a selected CLAP surface. For the author API, extension hooks,
and current limits, see the **[CLAP Guide](./CLAP-GUIDE.md)**,
the **[CLAP support matrix](CLAP-SUPPORT.md)** (implementation and host evidence), and
[wrapper implementation notes](./crates/aura-clap/README.md).

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
#    kind: effect (default) | effect-mono | analyzer | instrument
cargo aura new my-plugin
cd my-plugin

# 4) Build & install into the host search path
cargo aura install --clap --release

# 4b) Or stay in the loop: rebuild + reinstall on save
cargo aura watch --clap --hot

# 5) Check toolchain / AURA_PATH / clap-validator
cargo aura doctor

# 6) Optional: validate the installed .clap (requires clap-validator on PATH)
cargo aura validate path/to/my-plugin.clap
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
cargo aura preview --component 1      # pick component N (multi-UI files)
cargo aura preview --no-watch         # one-shot render, no hot reload
```

### CLI reference

```bash
cargo aura new <name> [--vst3] [--lv2] [--kind <k>]   # scaffold a plugin workspace
cargo aura init [path] [--vst3] [--lv2] [--kind <k>]  # scaffold into an existing empty dir
cargo aura add <name> [--vst3] [--lv2] [--kind <k>]   # add another plugin under plugins/<name>/
cargo aura add-ui <name>                              # shared Slint UI crate under crates/<name>/
cargo aura build  [--clap|--vst3|--lv2] [--release] [-plug <crate>…]
cargo aura install [--clap|--vst3|--lv2] [--release] [--hot] [-plug <crate>…]
cargo aura watch  [--clap|--vst3|--lv2] [--release] [-plug <crate>…] [--no-install] [--hot]
cargo aura preview [path] [--component N] [--no-watch]
cargo aura run [-plug <name> | path.clap] […]        # aura-host: load .clap, audio, MIDI, GUI
cargo aura preset list  [-plug <name> | path.clap]   # factory presets (discovery)
cargo aura preset pull  [-plug <name> | path.clap] --key <K> --out <file>
cargo aura doctor                                    # toolchain / AURA_PATH / clap-validator
cargo aura validate <path.clap>... [validator-options] # local clap-validator validate wrapper
```

`kinds`: `effect` (default) · `effect-mono` · `analyzer` · `instrument`. CLAP is always on;
`--vst3` / `--lv2` add the extra format. `install --hot` writes the `aura-hot` proxy
(`Name.clap` + `Name.impl.*`) so `watch` can replace DSP under a loaded host.

### Local CLAP validation

The development CLI's `validate` command delegates to `clap-validator validate`
on PATH. It does not build or install a plugin, download tooling, or run in CI.
Paths and validation options pass through as separate arguments; stdout/stderr
are inherited unchanged, including `--json`. Numeric exit codes pass through;
failure to launch, or termination without an exit code, returns 1.

```bash
# Install the maintainer fork at the checked source revision:
cargo install --git https://github.com/lxndrbe/clap-validator --rev f806ebaa06c7179889778bfbd1ee98b92639715c --locked clap-validator
cargo aura validate --help
cargo aura validate "path with spaces/my-plugin.clap" --json
cargo aura validate --version
cargo aura doctor
```

`doctor` reports the resolved executable path and its version. The validator's
version string does not prove its source commit; record the installation revision
when updating the fork. The pin above is a tooling baseline, not an automatic
update policy. Reinstall `cargo-aura` from this checkout if an older installed
CLI does not have the command. The published framework API baseline is 0.15.0.

### Build / test the framework

```bash
cargo build --workspace
cargo test --workspace          # on non-Linux: --exclude lx-aura-lv2
cargo clippy --workspace --all-targets
```

---

## Design principles

1. **Slint + baseview only** — renderer is a backend choice (`FemtoVG` / Skia / software); toolkit is not.
2. **CLAP first, thin formats** — one plugin logic API; VST3/LV2 only on the ship matrix; no format-shaped core.
3. **Framework layout** — `crates/` · `examples/` · `tools/` (product catalogs keep their own plugins outside AURA).
4. **One CLI:** **`cargo aura`** — `new`, `init`, `add`, `add-ui`, `build`, `install`, `watch`, `preview`, `run`, `preset`, `validate`, `doctor`
5. **KISS for humans and agents** — `aura.toml`, boring paths.

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
| `aura-hot` | CLAP hot-reload proxy (`cargo aura watch --hot`) |
| `aura-host` | Standalone dev host — load `.clap`, MIDI in, params, plugin GUI (CLI + Slint shell) |
| `lx-aura-test` (`aura_test`) | State round-trip + process smokes (`[dev-dependencies]`) |
| `cargo-aura` | Scaffold, build, install, watch, validate, doctor, preview, run |
| `aura-preview` | Slint UI preview (`cargo aura preview`) |

> Cross-plugin shared-memory IPC (spectrum/relay + CV) moved **out of AURA** — it now lives in the product catalog as **`lx-shm`** (`lx-audiolabs-dev`).

---

## Author surface (minimal)

Complete headless gain plugin; enable the `lx-aura` dependency's `clap` feature.
The [CLAP Guide](CLAP-GUIDE.md#minimal-clap-plugin) includes its standalone manifest.

```rust
use aura::prelude::*;

#[derive(Params)]
pub struct GainParams {
    #[param(id = 1, name = "Gain", range = "linear(-24, 24)", default = 0.0, unit = "db")]
    pub gain: FloatParam,
}

pub struct MyGain;

impl PluginLogic for MyGain {
    type Params = GainParams;
    type DspState = ();

    fn info() -> PluginInfo {
        let mut info = PluginInfo::new("My Gain", "Example", "0.1.0", "my-gain");
        info.clap_id = "com.example.my-gain";
        info
    }

    fn init(_params: &GainParams, _sample_rate: f64) {}

    fn reset(_state: &mut (), _params: &GainParams, _config: &AudioConfig) {}

    fn process(
        _state: &mut (),
        params: &GainParams,
        buffer: &mut AudioBuffer<'_, f32>,
        _context: &mut ProcessContext,
    ) -> ProcessStatus {
        let channels = buffer.num_inputs().min(buffer.num_outputs());
        for sample in 0..buffer.num_samples() {
            // Advance the smoother once per frame, shared by all channels.
            let gain = 10.0_f32.powf(params.gain.read() / 20.0);
            for channel in 0..channels {
                let input = buffer.input(channel)[sample];
                buffer.output(channel)[sample] = input * gain;
            }
        }
        for channel in channels..buffer.num_outputs() {
            buffer.output(channel).fill(0.0);
        }
        ProcessStatus::Continue
    }
}

aura::export!(MyGain);
```

Param IDs are **required and wire-stable** — reordering fields does not renumber automation.

More detail: the [published API reference](https://docs.rs/lx-aura/0.15.0/aura/),
local crate docs (`cargo doc -p lx-aura --features clap --open`), and
[crates/aura-clap/README.md](./crates/aura-clap/README.md).

---

## Status & maturity

| Area | State |
|------|--------|
| `PluginLogic` + `#[derive(Params)]` | done |
| CLAP process / params / state / GUI | implemented; [support and host evidence](CLAP-SUPPORT.md) |
| Sample-accurate automation + mono mod | done |
| Sidechain (one optional bus) + MIDI I/O | done |
| Aux out (one optional bus) | done (`BusLayout::with_aux`, `smoke-aux`) |
| Latency / remote-controls / tail / render | done |
| VST3 (Win/mac) | done (host smoke) |
| LV2 (Linux) process + UI extension | done (UI host smoke depends on host) |
| crates.io publish | **0.15.0** published (`lx-aura` and 12 supporting crates) |
| `clap.preset-load` + factory discovery | done (`factory_presets`) |
| Poly mod / note expression | done (CLAP → `ProcessContext.notes`; `NoteVoiceTable` + `NOTE_END`) |
| Native MIDI 2 process | done (`ProcessContext.ump` / `ump_out`; 7-bit `midi` remains) |
| `notes_out` / arp-seq path | done (CLAP native; VST3/LV2 map On/Off/Choke) |
| `aura-host` standalone dev host | done (CLI + Slint shell; MIDI in, params, plugin-GUI embed) |

Changelog: [CHANGELOG.md](./CHANGELOG.md). Releases are tagged `vX.Y.Z`.

---

## Contributing

Issues and PRs welcome on this framework repo.

- Keep the **scope** (Slint + CLAP-first ship matrix). Do not add AU/egui/AAX “just in case.”
- Prefer small, tested changes. One assert for non-trivial logic.
- Match workspace versioning: single version in root `Cargo.toml` + `CHANGELOG.md`.
- Follow [the release checklist](RELEASE.md) before publishing or tagging a release.
- CI (Forgejo, self-hosted): README/guide Rust examples and sample consistency
  on every PR; `fmt --check`, `clippy -D warnings`, and workspace doc-tests on code changes.
- API compatibility: manual [semver workflow](.forgejo/workflows/semver.yml),
  run before releases; see [coverage and local commands](RELEASE.md#api-compatibility-check).

```bash
cargo fmt --all
cargo test --locked -p aura-docs   # README + CLAP-GUIDE Rust samples and consistency
cargo test --workspace
cargo clippy --workspace --all-targets
```

---

## Acknowledgments

`aura-dsp` draws ideas from:

| Project | Link | Notes |
|---------|------|-------|
| **naad** (rust-old) | [MacCracken/naad](https://github.com/MacCracken/naad/tree/main/rust-old) | DSP graph / Faust-like ops |
| **fundsp** | [SamiPerttu/fundsp](https://github.com/SamiPerttu/fundsp) | Composable / SIMD-friendly `AudioUnit` graphs |
| **infinitedsp** | [infinitedsp](https://github.com/infinitedsp/infinitedsp) | Typed compile-time DSP graphs |

Also: [truce](https://github.com/truce-audio/truce), [CLAP](https://github.com/free-audio/clap), [Slint](https://slint.dev), and the Rust audio community.

---

## License

Copyright © 2026 LX Audiolabs

This project is free software under the **GNU General Public License v3.0 or later**.\
Distributing plugins that link AURA implies GPL obligations for that combined work. Selling with source is fine; closed-only ships are not the goal.

Third-party notes: **Slint** is triple-licensed; the default ship path uses its **GPLv3** option.
