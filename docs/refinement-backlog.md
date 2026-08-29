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

- [x] **Removed** (2026-08-29). Dead API; CLAP keeps `in_place_pair = INVALID`.
      Re-add + wire only when a product plugin needs in-place.

### 3. `midi_map` / `midi_cc` / `map_source_to_param`

- [x] **Demoted to hint** (2026-08-29). Attrs/`ParamInfo` kept; docs no longer
      claim VST3/LV2/AU wiring. Wrappers still ignore. Wire later only if a
      product needs host MIDI-learn hints.

### 4. Stale public docs (quick)

- [x] README status **0.9.x** → **0.11.x** (this pass).
- [x] `aura-editor` / `aura-midi` / `aura-core` transport+info “later” wording (this pass).
- [x] `aura-params`: drop “aura-loader” / AU·VST2·AAX / fake midi-binding claims.

---

## P1 — publishable surface coherence

### 5. Automation parity

- [x] Documented on `PluginLogic::process` (CLAP chunked / VST3 last-point /
      LV2 per-run). Sharing `chunked_process` into VST3 left for later if needed.

### 6. De-CLAP `PluginLogic` (thin_formats)

- [x] **Honesty pass:** CLAP-oriented hooks labeled on the trait. Full move to
      extension traits deferred (larger refactor; see P2 if crates.io nears).

### 7. Umbrella `aura` surface

- [x] Root-export codec / `layout_at` / `NoteNameEntry` / Tuning helpers.
- [x] Prelude expanded + documented (niche helpers kept on purpose).
- [x] Feature-gate `dsp` (default on).

### 8. Derive packaging invariant

- [x] Documented: depend on `aura` umbrella (`::aura::params::…` codegen stays).

### 9. `ProcessContext::clear_midi`

- [x] Renamed to `clear_events` (clears midi/notes/ump in+out).

### 10. LV2 honesty

- [x] Documented on `bus_layouts` + `aura-lv2` crate docs (first layout only;
      reduced subset).

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
- [x] `aura-params` sample.rs (2026-08-29).
- [ ] Optional re-scan later if more prose piles up.

---

## Suggested order of work

1. ~~P0~~ done  
2. ~~P1~~ done (extension-trait de-CLAP still optional later)  
3. ~~leftover deslop sample.rs~~ done  
4. P2 metadata / NOTICE only when crates.io is back on the table  

Tick boxes here as you go. When a chunk lands, one line in `CHANGELOG` under
Unreleased is enough — no second mega-doc.
