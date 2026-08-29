# crates.io prep (do not publish until checklist is green)

Stand: 2026-08-29 · Workspace **0.11.0**

**Policy:** We will not publish half-baked `0.0.x` noise. First publish = real
author surface, honest docs, Tier A+B metadata complete, dry-run clean.
`publish = false` stays until an explicit release decision.

Related: [`refinement-backlog.md`](./refinement-backlog.md),
[`licensing-compliance.md`](./licensing-compliance.md), root [`NOTICE`](../NOTICE).

---

## Blocker: crate name collisions

crates.io names are global. **`aura-core` is already taken** (fosskers / Arch
`aura` package manager, GPL-3.0-only, v0.8.x). Packaging dependents that resolve
`aura-core = "0.11"` will hit *their* crate, not ours, until we own a unique name.

**Before any upload**, pick a prefix and rename (or reserve unused names):

| Current | Suggested publish name (example) |
|---------|----------------------------------|
| `aura` | `lx-aura` or `aurafw` |
| `aura-core` | `lx-aura-core` (**required** — taken) |
| `aura-params` / `aura-derive` / … | same prefix |
| `aura-baseview` / `aura-editor` | same prefix (MIT UI stack) |

Internal path deps / repo layout can keep short names; publish names can differ
via `[package] name` only if we accept Cargo.toml rename. Prefer one rename pass
of the whole Tier A+B set under `lx-aura-*` (or similar) so docs stay consistent.

Until rename: metadata + `cargo package` on **leaf** crates only (`aura-params`,
`aura-derive`, `aura-midi`, `aura-build`, `aura-baseview`). Dependents fail
package resolve against crates.io — expected.

---

## Crate set

### Tier A — author + formats

| Crate | Role | License |
|-------|------|---------|
| `aura-params` | Params / smoothers | GPL-3.0-or-later |
| `aura-derive` | `#[derive(Params)]` | GPL-3.0-or-later |
| `aura-midi` | MIDI / UMP buffers | GPL-3.0-or-later |
| `aura-core` | `PluginLogic`, process, state | GPL-3.0-or-later |
| `aura-clap` | CLAP wrapper | GPL-3.0-or-later |
| `aura-vst3` | VST3 wrapper | GPL-3.0-or-later |
| `aura-lv2` | LV2 wrapper (reduced subset) | GPL-3.0-or-later |
| `aura` | Umbrella + features | GPL-3.0-or-later |

Authors normally depend on **`aura`** only (`features = ["clap", …]`).

### Tier B — UI + DSP + build

| Crate | Role | License |
|-------|------|---------|
| `aura-dsp` | Portable DSP (`dsp` feature on `aura`) | GPL-3.0-or-later (+ NOTICE) |
| `aura-build` | `slint-build` + `@aura` widgets / fonts | GPL-3.0-or-later (fonts OFL) |
| `aura-baseview` | Slint + baseview window stack | **MIT** |
| `aura-editor` | Host `Editor` adapter | **MIT** |

### Explicitly out (for now)

`aura-hot` (untested) · `aura-host` · `cargo-aura` · `aura-preview` ·
`aura-test` (dev-only) · smoke examples.

---

## Publish order (when flipping `publish = true`)

```text
aura-params → aura-derive → aura-midi → aura-core
  → aura-dsp → aura-clap → aura-vst3 → aura-lv2
  → aura-build → aura-baseview → aura-editor → aura
```

Same workspace version on every crate (`version.workspace = true`).
Workspace path deps also carry `version = "0.11.0"` so `cargo package` can
rewrite them to crates.io deps on publish.

**When bumping the workspace version**, update both
`[workspace.package] version` **and** every `version = "…"` on path deps in
`[workspace.dependencies]`.

---

## Dry-run (no upload)

`publish = false` blocks `cargo publish`. Package verification without upload:

```bash
cargo package -p aura-params --no-verify --allow-dirty
cargo package -p aura-derive --no-verify --allow-dirty
# … each Tier A/B crate …
```

Before a real publish: set `publish = true` only on the set above, then
`cargo publish -p <crate> --dry-run` in order (needs network + login).

---

## Checklist before first real publish

- [x] Tier A+B package metadata (keywords, categories, rust-version, homepage)
- [x] Per-crate README (or umbrella README for `aura`)
- [x] Root `NOTICE` (DSP provenance + fonts pointer)
- [x] P0/P1 refinement honesty (persist, dead APIs, surface docs)
- [ ] Human API freeze review (what is public vs `doc(hidden)`)
- [ ] At least one product (Ember/Nimbus) cut on this version without API thrash
- [ ] Flip `publish = true` + dry-run green + annotated tag
- [ ] Actual `cargo publish` in order

---

## Consumer example (after publish)

```toml
aura = { version = "0.11", features = ["clap", "dsp"] }
aura-baseview = { version = "0.11", features = ["backend-femtovg"] }
aura-editor = { version = "0.11", features = ["backend-femtovg"] }
aura-build = { version = "0.11" }  # [build-dependencies]
```

Until then: path/git deps as today.
