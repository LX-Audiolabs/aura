# crates.io prep

Stand: 2026-08-29 · Workspace **0.12.0** (git tag `v0.12.0`) ·
**Tier A+B+C published to crates.io** as `lx-aura-*` 0.12.0
(incl. `lx-aura-test`).

**Policy:** No half-baked `0.0.x` noise. First registry upload was **0.12.0**
after rename, freeze, and product soak.

Related: [`refinement-backlog.md`](./refinement-backlog.md),
[`licensing-compliance.md`](./licensing-compliance.md), root [`NOTICE`](../NOTICE).

---

## Naming: `lx-aura-*` packages, `aura_*` libs

crates.io names are global. **`aura-core` is taken** (fosskers / Arch `aura`
package manager). Tier A+B therefore ship as:

| Package (`[package] name`) | Rust crate (`[lib] name`) | Role |
|----------------------------|---------------------------|------|
| `lx-aura` | `aura` | Umbrella |
| `lx-aura-core` | `aura_core` | `PluginLogic`, process, state |
| `lx-aura-params` | `aura_params` | Params / smoothers |
| `lx-aura-derive` | `aura_derive` | `#[derive(Params)]` |
| `lx-aura-midi` | `aura_midi` | MIDI / UMP buffers |
| `lx-aura-clap` / `-vst3` / `-lv2` | `aura_clap` / … | Format wrappers |
| `lx-aura-dsp` | `aura_dsp` | Portable DSP |
| `lx-aura-build` | `aura_build` | Slint `@aura` + fonts |
| `lx-aura-baseview` | `aura_baseview` | Slint + baseview (**MIT**) |
| `lx-aura-editor` | `aura_editor` | Host `Editor` adapter (**MIT**) |
| `lx-aura-test` | `aura_test` | Dev-dep test helpers |

Repo folders stay `crates/aura-*`. Authors write `use aura::…` in Rust;
Cargo.toml deps use the `lx-aura-*` keys (see consumer example below).

---

## Crate set

### Tier A — author + formats

Authors normally depend on **`lx-aura`** only (`features = ["clap", …]`).

License: GPL-3.0-or-later for the stack above except baseview/editor (MIT).

### Tier B — UI + DSP + build

`lx-aura-dsp` (via `dsp` feature) · `lx-aura-build` · `lx-aura-baseview` ·
`lx-aura-editor`.

### Tier C — test helpers (dev-dep)

`lx-aura-test` (`aura_test`) — state round-trip / process smoke. Publish with
A+B; plugins and scaffolds use it as `[dev-dependencies]` only.

### Explicitly out (for now)

`aura-hot` (untested) · `aura-host` · `cargo-aura` · `aura-preview` ·
smoke examples. Package names unchanged.

---

## Publish order (when flipping `publish = true`)

```text
lx-aura-params → lx-aura-derive → lx-aura-midi → lx-aura-core
  → lx-aura-dsp → lx-aura-clap → lx-aura-vst3 → lx-aura-lv2
  → lx-aura-build → lx-aura-baseview → lx-aura-editor → lx-aura
  → lx-aura-test   # after core+params (dev helper)
```

Same workspace version on every crate (`version.workspace = true`).
Workspace path deps also carry `version = "0.12.0"` so `cargo package` can
rewrite them to crates.io deps on publish.

**When bumping the workspace version**, update both
`[workspace.package] version` **and** every `version = "…"` on path deps in
`[workspace.dependencies]`.

---

## Dry-run (no upload)

`publish = false` blocks `cargo publish`. Package verification without upload:

```bash
cargo package -p lx-aura-params --no-verify --allow-dirty
cargo package -p lx-aura-derive --no-verify --allow-dirty
# … each Tier A/B crate …
```

Before a real publish: set `publish = true` only on the set above, then
`cargo publish -p <crate> --dry-run` in order (needs network + login).

---

## Checklist before first real publish

- [x] Tier A+B package metadata (keywords, categories, rust-version, homepage)
- [x] Per-crate README (or umbrella README for `lx-aura`)
- [x] Root `NOTICE` (DSP provenance + fonts pointer)
- [x] P0/P1 refinement honesty (persist, dead APIs, surface docs)
- [x] Package rename to `lx-aura-*` (lib names kept)
- [x] Human API freeze review — signed off 2026-08-29: `aura::prelude` +
      format exports = author API; `aura-hot` out; LV2 subset / MIDI-learn hint
      honesty already documented. Product rebuild (Aether/Meridian/Equilibrium/
      Loom/Ember/Nimbus + …) + Reaper smoke = no API thrash on 0.12.
- [x] Product soak on this version (same plugins; `lx-aura-*` deps; Reaper OK)
- [x] Annotated git tag `v0.12.0` (repo release)
- [x] Flip `publish = true` on Tier A+B
- [x] Actual `cargo publish` in order — **0.12.0** on crates.io (2026-08-29)

---

## Consumer example (after publish)

```toml
lx-aura = { version = "0.12", features = ["clap", "dsp"] }
lx-aura-baseview = { version = "0.12", features = ["backend-femtovg"] }
lx-aura-editor = { version = "0.12", features = ["backend-femtovg"] }
lx-aura-build = { version = "0.12" }  # [build-dependencies]
```

```rust
use aura::prelude::*;
// aura_editor / aura_build via their lib names
```

Until then: path/git deps as today (`cargo aura new` emits path deps on
`lx-aura` / `lx-aura-editor` / `lx-aura-build`).
