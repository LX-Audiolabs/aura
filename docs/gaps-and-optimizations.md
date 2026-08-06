# AURA — gaps & optimizations

Living list of **known gaps** (vs product cutover / basis DoD) and **optimizations** (keep, harden, or defer).  
Roadmap order still lives in [migration-steps.md](./migration-steps.md). Update both when a gap closes.

Last pass: 2026-08-06 (post `aura-derive` review).

---

## Principles (do not re-litigate)

| Keep | Drop / never |
|------|----------------|
| `#[derive(Params)]` for author UX | Full truce-derive dump (`plugin_info!`, LV2 TTL sidecars, `#[derive(State)]` in framework) |
| Explicit param `id = N` (wire-stable) | Silent ID auto-assign that renumbers on reorder |
| Thin format wrappers | Format-shaped `aura-core` |
| Product DSP in product repo | Absorbing `lx-dsp` / vault into AURA |
| Dependabot visibility + manual merge | Auto-merge Slint / clap-sys |

**Derive is required** for product cutover — not optional polish. Hand-written `Params` is only for microscopic smokes (and even smoke-gain now uses derive).

---

## Closed recently

| Item | Evidence |
|------|----------|
| `aura-derive` (`Params`, `ParamEnum`) | Crate + `crates/aura/tests/derive_params.rs` (12 tests) + smoke-gain |
| Selective port (no State / plugin_info / LV2 meta in derive) | Crate docs; intentional |
| agal orientation in framework workspace | `agal.toml`, `agal/notes/_workspace.md`, `07-aura/aura-scope` |
| Dependabot (AURA only) | `.github/dependabot.yml` |

---

## Gaps — product cutover (lx-audiolabs-plugins → AURA)

### G1 — Explicit param IDs (syntax delta)

| | |
|--|--|
| **Today (truce product)** | Often `#[param(name = …, range = …)]` **without** `id` (auto-assign) |
| **AURA derive** | **`id = N` required** — missing → compile error |
| **Why AURA differs** | Wire-stable automation / state; no renumber on field reorder |
| **Cutover work** | Assign stable IDs once per plugin; document mapping if old sessions exist |
| **Severity** | **Hard** — every product `Params` struct |

### G2 — No generated `*ParamId` enum

| | |
|--|--|
| **Today (truce product)** | Editors use `AetherParamsParamId as P` → `P::Eq1Freq` |
| **AURA derive** | **No** `*ParamId` enum (intentionally dropped with truce-derive extras) |
| **Options** | **A)** Extend derive to emit optional `FooParamsParamId` (product ergonomics) · **B)** Migrate editors to `const` / raw `u32` / field accessors |
| **Decision** | **Open** — pick before Stage 7 pilot |
| **Severity** | **Hard** for mechanical cutover of existing `editor.rs` |

### G3 — Multi-format (VST3 / LV2)

| | |
|--|--|
| **Product** | `clap` + `vst3` + `lv2` features, dist zips |
| **AURA** | CLAP only (`aura-clap`) |
| **Work** | Stage 5 thin wrappers + `cargo aura build/install --vst3|--lv2` |
| **Severity** | **Hard** for full catalog ship matrix |

### G4 — Host GUI proof

| | |
|--|--|
| **AURA** | `clap.gui` + smoke-gain UI; clap-validator path green |
| **Missing** | Bitwig (then REAPER) parented open on smoke-gain |
| **Severity** | **P0 basis** — validator ≠ host |

### G5 — State / presets surface

| | |
|--|--|
| **Product** | Custom state, vault presets, migration helpers |
| **AURA** | CLAP state as flat LE param blob; `#[persist]` fields in derive for small UI state |
| **Work** | Plugin-level save/load hooks if host blob must carry more than params; vault stays product |
| **Severity** | Soft if params-only restore is enough; hard if product needs rich blobs day one |

### G6 — Editor product layer

| | |
|--|--|
| **Product** | `lx-slint-editor` (zoom, ticks, chrome), `lx-ui-slint` |
| **AURA** | Thin `AuraSlintEditor` + `@aura` widgets |
| **Work** | After cutover: port editor helpers into product crates (or optional `aura-*` only if shared by ≥2) |
| **Severity** | Soft for framework basis; hard for 1:1 UI parity |

### G7 — Process surface (MIDI / buses)

| | |
|--|--|
| **Product / truce** | Note events, richer buses as needed |
| **AURA** | Stereo-fixed CLAP; transport on `ProcessContext`; no note-ports yet |
| **Work** | Add only when smoke/pilot needs (analyzers/FX mostly soft) |
| **Severity** | Low for current catalog |

### G8 — Scaffold / install polish

| | |
|--|--|
| **Today** | `cargo aura new` basic; install path exists |
| **Missing** | Product-shaped layout; clean `.clap` naming; scaffold uses `#[derive(Params)]` + explicit ids |
| **Severity** | P1 basis / author UX |

---

## Gaps — framework quality (not cutover blockers)

| ID | Gap | Notes |
|----|-----|--------|
| **F1** | `aura-derive` is one large `lib.rs` (~2.6k LOC) | Split later: parse / gen / param_enum — maintainability only |
| **F2** | CLAP extensions incomplete vs hosts | latency, note-ports, etc. only when needed |
| **F3** | clap-sys may lag free-audio revision | Policy already in `aura-clap` README; Dependabot tracks crates.io |
| **F4** | femtovg/Skia mostly transitive via Slint | Upgrade via Slint group PRs |
| **F5** | Docs lag code | Keep this file + migration-steps status in sync after each stage close |

---

## Optimizations (ranked)

### Do soon (high ROI)

1. **Land + lock derive API** — commit workspace member, smoke-gain, tests; freeze “explicit `id` required” as public contract.  
2. **Decide G2 (ParamId)** before writing pilot migration scripts.  
3. **Bitwig GUI smoke** (G4) — unblocks calling editor path “done”.  
4. **Scaffold emits derive + `id`s** — new plugins never learn hand-`Params`.  
5. **`install --clap` artifact name** — fewer manual rename steps.

### Do for Stage 5 / cutover

6. Thin **VST3** then **LV2** (G3).  
7. Pilot plugin: pin param IDs, apply ParamId decision, path-dep AURA.  
8. State hooks only if pilot presets need more than param + `#[persist]` (G5).

### Defer (Stage 6 / polish)

9. Split `aura-derive` modules (F1).  
10. `cargo aura init` / aura-gui / kind templates.  
11. Note-ports / multi-bus unless a concrete plugin needs them (G7).  
12. agal Dependabot (framework first; agal later).

### Avoid

- Replacing derive with builders/codegen scripts for product scale.  
- Auto-assign param IDs “like truce” without a migration story (breaks wire stability).  
- Porting truce-derive State/plugin_info into AURA “for completeness”.  
- Early bulk cutover of all six plugins before basis + G2 decision.

---

## Cutover checklist (pilot plugin)

When Stage 7 starts on **one** plugin (e.g. aether):

- [ ] AURA path/git dep; `use aura::prelude::*`  
- [ ] Every `#[param(...)]` has stable **`id = N`** (G1)  
- [ ] ParamId strategy applied (G2 option A or B)  
- [ ] Formats needed for that release (CLAP minimum; VST3/LV2 if shipping)  
- [ ] Editor compiles against `AuraSlintEditor` / product UI crates  
- [ ] State/presets: param blob + product vault still work  
- [ ] clap-validator + host smoke on the pilot  
- [ ] No product DSP moved into AURA

---

## See also

| Doc | Role |
|-----|------|
| [migration-steps.md](./migration-steps.md) | Strategy, stages, DoD, work order |
| [licensing-compliance.md](./licensing-compliance.md) | GPL / Slint / ship matrix |
| `agal/notes/_workspace.md` | Durable atoms for agents |
| `agal/skills/07-aura/aura-scope.md` | Scope loadout |
| `crates/aura-derive` | Derive implementation + intentional non-goals in crate docs |
