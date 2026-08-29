# AURA — refinement backlog (pre–crates.io)

Stand: 2026-08-29 · Workspace **0.11.0**

Piecewise checklist after a publishable-surface review. **crates.io stays deferred**
until this list is in better shape and we trust the author API.

**Not AURA work:** real-synth polish → **Ember** (product); real-FX → **Nimbus**
in `lx-audiolabs-dev`. Packaging/signing → `lx-audiolabs-plugins`. CI already
runs on GitHub push.

Canonical elsewhere (do not duplicate specs here):

| Topic | Where |
|-------|--------|
| Host proofs / CLAP leftovers | `crates/aura-clap/README.md` |
| Cross-cutting open | `agal/notes/_workspace.md` |
| Local scratch | `docs/optimisation-todo.md` (gitignored) |
| SemVer | `docs/versioning.md` |

---

## Policy

- [x] **crates.io deferred** — first publish only after P0 honesty + a deliberate
  API freeze review. Keep `publish = false`.
- [x] Product DSP (Ember / Nimbus) out of this backlog.

---

## P0 — honesty / data-loss risks (do first)

### 1. `#[persist]` not in host state

- [x] **Wired** (2026-08-29): `encode_state` / `decode_state` use v2 envelope
      (`AURA` + ver 2 + params + persist). Formats already call these helpers.
      Legacy v1 blobs still decode. Tests in `aura-core` state + `aura` derive.

### 2. `PluginLogic::supports_in_place`

Public method; CLAP always sets `in_place_pair = CLAP_INVALID_ID`. Dead contract.

- [ ] Implement for CLAP (and VST3 if applicable), **or** remove from trait.

### 3. `midi_map` / `midi_cc` / `map_source_to_param`

Docs claim VST3 `IMidiMapping` and LV2 `midi:binding`. No format callers.

- [ ] Implement bindings, **or** strip public claims / demote to “hint only,
      unused by wrappers”.

### 4. Stale public docs (quick)

- [x] README status **0.9.x** → **0.11.x** (this pass).
- [x] `aura-editor` / `aura-midi` / `aura-core` transport+info “later” wording (this pass).
- [ ] `aura-params`: drop “aura-loader” / AU·VST2·AAX claims unless those formats ship.

---

## P1 — publishable surface coherence

### 5. Automation parity

CLAP: sample-accurate `CHUNKED` splits. VST3: last-point-per-block.
LV2: control ports per run.

- [ ] Document explicitly on `PluginLogic::process` / params, **or** share more
      of `chunked_process` into VST3.

### 6. De-CLAP `PluginLogic` (thin_formats)

`note_names`, param-indication, tuning hooks are CLAP-shaped on the core trait.

- [ ] Move to extension traits / format-local hooks; keep core format-neutral
      where possible (`agal.toml` `thin_formats`).

### 7. Umbrella `aura` surface

- [ ] Root-export `encode_state` / `decode_state` / `layout_at` / `NoteNameEntry`
      (and Tuning helpers authors need).
- [ ] Rebuild `prelude` as the real author set (trim niche or document why).
- [ ] Feature-gate `aura-dsp` so params-only plugins need not pull FFT/ebur128.

### 8. Derive packaging invariant

Generated code uses `::aura::params::…` — crates cannot use derive + params
without the umbrella.

- [ ] Emit `::aura_params::` (or document “must depend on `aura`” as required).

### 9. `ProcessContext::clear_midi`

Name clears only part of the event set.

- [ ] Rename to `clear_events` and clear all, or document the narrow behavior.

### 10. LV2 honesty

First `bus_layouts()` entry only; no latency/tail/presets/transport parity.

- [ ] Document “LV2 = reduced subset” on `bus_layouts` + LV2 README, **or**
      fill gaps that matter for Linux ship.

---

## P2 — polish / crates.io prep (later)

- [ ] Export naming: keep `export!`, add `export_clap!` alias for symmetry.
- [ ] Per-crate metadata: keywords, categories, readme, `rust-version`, docs.rs features.
- [ ] NOTICE / provenance for `aura-dsp` (naad and friends) vs GPL-or-later workspace.
- [ ] MIT (`aura-baseview` / `aura-editor`) vs GPL story written once for consumers.
- [ ] `ParamEventQueue` full-drop behavior documented.
- [ ] Split fat `aura-clap` modules further.
- [ ] README / crate intros: finish de-slop pass (see below).

---

## De-slop (human-facing prose)

Target register: **technical README / crate docs** — short claims, no throat-clearing.

Worst cluster today: root `README.md` Scope + Acknowledgments (em-dash mottos,
“stand on the shoulders”, padded table blurbs). Also:

| Location | Issue |
|----------|--------|
| README Scope / CLAP pitch | motto cadence, “not X” framing |
| Acknowledgments | “directly shaped / thanks…” — table alone enough |
| `aura-params` `sample.rs` | over-narrated sealed-trait essay |
| `aura-clap` README “not basis holes” | contrast filler |
| Stale “later” module docs | honesty, not just style |

Checklist:

- [x] README Scope + ship-matrix copy (this pass).
- [x] Acknowledgments: keep links/table; cut fluff (this pass).
- [x] Several crate `//!` “later” headers (this pass).
- [ ] `aura-params` sample.rs / param_infos_static over-narration.
- [ ] Re-scan after further edits (fuck-slop verify loop).

---

## Suggested order of work

1. ~~P0.1 persist~~ done  
2. P0.2 / P0.3 kill-or-implement dead APIs  
3. P0.4 remaining (`aura-params` ghost-format docs) + leftover de-slop  
4. P1.5–7 umbrella / prelude / dsp feature  
5. P1.5 automation docs + P1.10 LV2 honesty  
6. P2 metadata only when crates.io is back on the table  

Tick boxes here as you go. When a chunk lands, one line in `CHANGELOG` under
Unreleased is enough — no second mega-doc.
