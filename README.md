# AURA

[![License](https://img.shields.io/badge/license-GPL--3.0--or--later-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.96+-orange.svg)](rust-toolchain.toml)
[![Slint](https://img.shields.io/badge/UI-Slint-2379F4.svg)](https://slint.dev)
[![agal](https://img.shields.io/badge/powered%20by-agal-00ADD8.svg)](https://github.com/LX-Audiolabs/agal)
[![AI](https://img.shields.io/badge/dev-AI--assisted-6E40C9.svg)](https://github.com/LX-Audiolabs/agal)

**Audio Unified Rust Architecture**  
*(DE-Gag: **AU**dio **RA**hmenwerk)*

CLAP-first plugin framework for **LX Audiolabs**.  
Partner tooling: **[agal](https://github.com/LX-Audiolabs/agal)** (agent orientation) · this repo (runtime + formats + build + CLI).

**License:** [GPL-3.0-or-later](./LICENSE) — see [docs/licensing-compliance.md](./docs/licensing-compliance.md).

| Layer | Name |
|-------|------|
| Product | **AURA** |
| Crates | **`aura-*`** |
| Umbrella | **`aura`** → `use aura::*` |
| CLI package | **`cargo-aura`** |
| Invoke | **`cargo aura …`** (e.g. `cargo aura new`, `cargo aura build --clap -plug <crate> [<crate>...]`) |
| Config | **`aura.toml`** |

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

## Design principles

1. **Slint + baseview only** — renderer is a backend choice (FemtoVG / Skia / software); toolkit is not.
2. **CLAP first, thin formats** — one plugin logic API; VST3/LV2 only on the ship matrix; no format-shaped core.
3. **Framework layout** — `crates/` · `examples/` · `tools/` (product catalogs keep their own `plugins/` outside AURA).
4. **One CLI:** **`cargo aura`** — parity with `cargo truce` (`new`, `build`, `install`, `doctor`).
5. **KISS for humans and agents** — `aura.toml`, boring paths; orientation in **agal**.

---

## License

Copyright © 2026 LX Audiolabs  

This project is free software under the **GNU General Public License v3.0 or later**.  
Distributing plugins that link AURA implies GPL obligations for that combined work. Selling with source is fine; closed-only ships are not the goal.

Third-party notes (including **Slint** triple license — default ship path **GPLv3**):  
[docs/licensing-compliance.md](./docs/licensing-compliance.md).
