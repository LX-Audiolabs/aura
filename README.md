# AURA

[![License](https://img.shields.io/badge/license-GPL--3.0--or--later-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.96+-orange.svg)](rust-toolchain.toml)
[![Slint](https://img.shields.io/badge/UI-Slint-2379F4.svg)](https://slint.dev)
[![agal](https://img.shields.io/badge/powered%20by-agal-00ADD8.svg)](https://github.com/LX-Audiolabs/agal)

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
| Invoke | **`cargo aura …`** (e.g. `cargo aura new`, `cargo aura build --clap`) |
| Config | **`aura.toml`** |

---

## Scope (hard)

AURA is **intentionally narrow**. If that is not your stack, use something else — no hard feelings.

| We commit to | We do **not** ship |
|--------------|--------------------|
| **UI:** [Slint](https://slint.dev) **only**, via **`aura-baseview`** (+ **`aura-editor`** host adapter) | egui, iced, Vizia as first-class UIs |
| **Formats:** **CLAP**, **VST3**, **LV2** | Audio Units (AU), AAX, VST2 |
| **Hosts:** Bitwig-first, then REAPER and other CLAP/VST3/LV2 hosts | Pro Tools / Logic-only AU pipelines as a goal |
| **Platforms:** Windows, Linux, macOS | iOS / embedded plugin hosts in v1 |

**CLAP is primary.** VST3 and LV2 are supported; they are not an excuse to grow a kitchen-sink framework.

### Slint + baseview (always)

- **Always:** Slint UI + **baseview** host window (embed, scale, keys, clipboard, …).
- **Choose renderer** (features / project config), not toolkit:
  - **FemtoVG** (OpenGL) — default  
  - **Skia** — optional  
  - **Software / wgpu blit** — optional  
- There is no “raw egui editor” mode and no second UI framework in AURA.

That stack is **`aura-baseview`** (window/renderer) + **`aura-editor`** (host adapter) + **`aura-build`** (compile-time). Every AURA plugin UI goes through it — not an optional addon.

### Looking for egui, iced, or AU?

AURA is **not** the right framework. Please use other open source projects that already optimize for that:

| Need | Go here |
|------|---------|
| Flexible Rust plugins, egui / iced / multiple UI styles, broad format matrix | **[nice-plug](https://codeberg.org/RustAudio/nice-plug)** (NIH-plug successor) |
| CLAP-centric Rust ecosystem / lower-level CLAP work | **[clack](https://github.com/prokopyl/clack)** (and related CLAP crates) |
| Full multi-format framework including AU/AAX paths, egui/iced/Vizia options | **[truce](https://github.com/truce-audio/truce)** · [truce.audio](https://truce.audio) |

We stand on the shoulders of that work (and permissive crates like CLAP bindings). AURA exists so **LX** can own a **Slint + baseview + CLAP/VST3/LV2** line without carrying every UI toolkit and every legacy format.

---

## Design principles

1. **Slint + baseview only** — renderer is a backend choice (FemtoVG / Skia / software); toolkit is not.
2. **Thin formats** — one plugin logic API, three wrappers; no format-shaped core.
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
