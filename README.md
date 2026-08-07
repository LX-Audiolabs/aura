# AURA

**Audio Unified Rust Architecture**  
*(DE-Gag: AUdio RAhmenwerk)*

CLAP-first plugin framework for **LX Audiolabs**.  
Partner tooling: **[agal](https://github.com/LX-Audiolabs/agal)** (agent orientation) · this repo (runtime + formats + build + CLI).

**License:** [GPL-3.0-or-later](./LICENSE) — see [docs/licensing-compliance.md](./docs/licensing-compliance.md).

| Layer | Name |
|-------|------|
| Product | **AURA** |
| Crates | **`aura-*`** (migrating from today’s `lx-*` / truce) |
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

AURA is **not** the right framework. Please use projects that already optimize for that:

| Need | Go here |
|------|---------|
| Flexible Rust plugins, egui / iced / multiple UI styles, broad format matrix | **[nice-plug](https://codeberg.org/RustAudio/nice-plug)** (NIH-plug successor — NIH being retired; RustAudio Discord) |
| CLAP-centric Rust ecosystem / lower-level CLAP work | **[clack](https://github.com/prokopyl/clack)** (and related CLAP crates) |
| Full multi-format framework including AU/AAX paths, egui/iced/Vizia options | **[truce](https://github.com/truce-audio/truce)** · [truce.audio](https://truce.audio) |

We stand on the shoulders of that work (and permissive crates like CLAP bindings). AURA exists so **LX** can own a **Slint + baseview + CLAP/VST3/LV2** line without carrying every UI toolkit and every legacy format.

---

## Strategy

**Finish AURA first** (install & build plugins like truce), **then** re-point lx-audiolabs-plugins.

```text
learn from our plugins  →  build a tighter framework (AURA)
AURA works end-to-end   →  only then migrate product plugins
```

truce and the current plugin tree are **design references**, not a dump target.  
AURA ≈ “truce-shaped workflow, LX-shaped product decisions” (Slint+baseview, CLAP/VST3/LV2).

**Basis fertig** (gate before product cutover): `cargo aura new/build/install`, CLAP smoke in `examples/`, Slint UI path, clap-validator — see [docs/migration-steps.md](./docs/migration-steps.md).

## Design principles

1. **Slint + baseview only** — renderer is a backend choice (FemtoVG / Skia / software); toolkit is not.
2. **Thin formats** — one plugin logic API, three wrappers; no format-shaped core.
3. **Framework layout** — `crates/` · `examples/` · `tools/` (product catalogs keep their own `plugins/` outside AURA).
4. **No early product migration** — AURA must stand alone first.
5. **One CLI:** **`cargo aura`** — parity with `cargo truce` (`new`, `build`, `install`, `doctor`).
6. **KISS for humans and agents** — `aura.toml`, boring paths; orientation in **agal**.

---

## Repository layout

```text
AURA/
  README.md
  LICENSE                   # GPL-3.0-or-later
  Cargo.toml                # workspace root
  docs/
    licensing-compliance.md
    migration-steps.md
  crates/                   # framework only (aura-*)
  examples/                 # framework smoke / demos (not the LX product catalog)
  tools/                    # cargo-aura → `cargo aura`
```

| Path | Owns |
|------|------|
| `crates/` | Runtime, params, formats, **`aura-baseview`**, **`aura-editor`**, **`aura-build`** |
| `examples/` | In-tree smoke / demos to prove the framework |
| `tools/` | **`cargo-aura`** → `cargo aura …` |
| `docs/` | License, strategy, migration order |

**Product plugins** (aether, lucent, …) live in **lx-audiolabs-plugins** — cut over only after AURA basis is done.

---

## Status

Building the **framework basis** (not product migration).

| Crate | Role |
|-------|------|
| [`aura`](./crates/aura) | Umbrella — `use aura::prelude::*`, feature `clap` |
| [`aura-params`](./crates/aura-params) | Params / ranges / smoothers |
| [`aura-derive`](./crates/aura-derive) | `#[derive(Params)]` / `ParamEnum` + `<Struct>ParamId` |
| [`aura-core`](./crates/aura-core) | `PluginLogic`, `Editor`, process (thin) |
| [`aura-clap`](./crates/aura-clap) | CLAP export (`aura::export!`) — **free-audio/clap** via clap-sys |
| [`aura-baseview`](./crates/aura-baseview) | Slint + baseview window stack (MIT; crates.io **later**) |
| [`aura-editor`](./crates/aura-editor) | Host Editor adapter (re-exports baseview today) |
| [`aura-build`](./crates/aura-build) | `@aura` widgets + fonts |
| [`cargo-aura`](./tools/cargo-aura) | CLI — `cargo aura new\|build\|install\|doctor` |
| [`examples/smoke-gain`](./examples/smoke-gain) | CLAP smoke (gain); clap-validator **0 failed** |

```bash
cargo install --path tools/cargo-aura --force
cargo aura doctor
cargo build -p smoke-gain --release
# clap-validator validate target/release/smoke_gain.dll  # or rename .clap
```

**Still need for basis:** VST3/LV2 host smoke.  
(Bitwig CLAP green; VST3+LV2 wrappers + install in-tree — confirm in a DAW.)  
**`aura-derive`:** landed (explicit `id = N`, `<Struct>ParamId` enum — G2 option A); `cargo aura new` scaffolds a validator-green CLAP plugin; product cutover gaps → [docs/gaps-and-optimizations.md](./docs/gaps-and-optimizations.md).

**Roadmap:** [docs/migration-steps.md](./docs/migration-steps.md) · **Gaps / opts:** [docs/gaps-and-optimizations.md](./docs/gaps-and-optimizations.md)  
Historical naming: `../lx-framework-plan.md`.

---

## License

Copyright © 2026 LX Audiolabs  

This project is free software under the **GNU General Public License v3.0 or later**.  
Distributing plugins that link AURA implies GPL obligations for that combined work. Selling with source is fine; closed-only ships are not the goal.

Third-party notes (including **Slint** triple license — default ship path **GPLv3**):  
[docs/licensing-compliance.md](./docs/licensing-compliance.md).
