# AURA — framework improvement list

Stand: **2026-11-10** · Workspace **0.14.3** + Unreleased · Evidence: Helmira **M4–M5b** on `main`

Actionable list of framework-side improvements (not product polish). Spec detail
stays in the linked files — this is the **priority roll-up**.

**Policy:** gap → failing consumer/AURA test → small additive PR. No “just in
case” API. RT after init: no alloc / lock / file / graph rebuild.

Canonical elsewhere:

| Topic | Where |
|-------|--------|
| Helmira-driven M3/M4 gaps | [`helmira-driven-gaps.md`](./helmira-driven-gaps.md) |
| Pre-publish / P2 polish | [`refinement-backlog.md`](./refinement-backlog.md) |
| CLAP host proofs | `crates/aura-clap/README.md` |
| SemVer / release | [`versioning.md`](./versioning.md) |
| Step-seq / arp (later) | [`step-sequencer-arp-clap-timing.md`](./step-sequencer-arp-clap-timing.md) |
| crates.io | [`crates-io-prep.md`](./crates-io-prep.md) |

**Helmira consumer state (2026-11-10):** M1–M5b shipped on product `main`.
DSP uses existing primitives (SVF, shelves, formant vowel morph, Distortion,
`DelayLine`, tempo helpers, `Reverb`, `unison_voice_pan_gains`,
`ModRouteTable`+scale, `SmoothRandom`). Product-local: stutter buffer, mono
arp, 1-pole DC snap, SinFold→WaveFold map. AURA now provides RT-safe reverb
decay/damping setters and a shared `DcBlocker`; Helmira still needs its separate
product wiring to use them live.

---

## A — Ship / hygiene (do when convenient)

| # | Item | Why | Notes |
|---|------|-----|--------|
| A1 | Cut **0.14.4** | Land Unreleased DSP (tempo, Lfo phase, `ModRouteTable`+scale, blend, pan, `SmoothRandom`) + any new F* items | Bump workspace version + CHANGELOG + annotated tag. Helps Helmira path-dep. |
| A2 | Push local `main` → Forgejo | Docs commits may sit local-only | No GitHub direct push from this checkout. |
| A3 | Delete stale `feature/mod-route-scale` | Scale merged (PR #18); checklist ticked | After remote sees main. |
| A4 | Helmira path-dep on next cut | Checklist leftover in gaps.md | Product repo; not AURA code. |

## F — Post-M5 evidence (do next; measured in Helmira)

Items with **failing product honesty or repeated product code**. Highest first.

| # | Item | Why now | API / contract sketch | Non-goal |
|---|------|---------|------------------------|----------|
| **F1 — done** | **`Reverb::set_decay` / `set_damping` (RT-safe)** | Helmira IDs 172–173 knobs bind + state/preset OK, but `make_reverb` only runs at init/reset. Process only mutates `reverb.mix`. User turns damp/fb → no live effect until reset. | `set_decay(f32)` → clamp 0..0.99, write comb `feedback` on all L/R combs **in place**. `set_damping(f32)` → clamp 0..1, write `damp1`/`damp2` (or equivalent) on all combs. Optional: `set_pre_delay_ms` only if length already reserved at `new` (no realloc). Keep `pub mix` / `pub width`. | `Reverb::new` on audio thread; realloc comb buffers; Helm comb-length dump; make decay/damping `pub` fields without updating internal comb state |
| **F2 — done** | **`DcBlocker` 1-pole** | Helmira ships always-on local DC with exact-zero snap for dry silence tests. Cheap shared util. | `DcBlocker { x1, y1 }` + `process(x) -> y`; R default 0.995; bad coefficients fall back to default; non-finite input becomes 0; denormals flushed. **No** silent exact-zero hard-clear (Helmira snap stays product policy). | Replace full HP EQ design; force all effects through DC |
| F3 | **`DistortionType::SinFold` (or fold sine law)** | Helmira type 3 → WaveFold shortcut until A/B. | Additive enum variant + pure transfer + tests; wire in `Distortion::process_sample` | Rename product UI types; bit-identical mopo |
| F4 | **`WhiteNoise` / noise seed API** | Helmira shortcut: AURA fixes seed 42 → voices correlate. | `from_seed(u64)` or `reseed`; keep default deterministic for tests | Crypto RNG; per-sample heap |
| F5 | **ParamMod / monophonic `modulatable` honesty helpers** | Helmira advertises CLAP `modulatable` / per-note flags; DSP reads effective values only on a subset. Framework can document + optional helper for “flag only if voice reads effective”. | Docs in `aura-clap` + maybe test helper asserting param flag ↔ `effective_value` use | Auto-magic modulation engine |
| F6 | **Factory preset / state Blob defaults pattern** | Helmira Init broken when empty `Values(&[])`; fixed as Blob of defaults. If more products hit this, document + example in export/state. | Docs + example factory preset Blob roundtrip test pattern in `aura-clap` / plugin template | Mandate one preset format |
| F7 | **LFO digital / non-sine factory shapes** | Helmira maps Helm waves 5–10 → sine, noise waves stay S&H; product may add local shapes later. | Only if second product needs same shapes: additive LFO shape variants or wavetable LFO helper | Helm name table in public API |
| F8 | **Sub PWM / phase-warp primitive** | Helmira `sub_shuffle` unused until PWM exists. | Tiny phase-distort or pulse-width helper on osc path, RT-safe | Full PD/VPS synth |
| F9 | **Unison >8 + harmonize / per-voice CM** | Helmira keeps product unison extras; AURA `UnisonOscillator` caps 8, 1/N, no harmonize. | Promote only if second product needs same law beyond `unison_voice_pan_gains` | Force all products onto one unison type |
| F10 | **`DelayLine` / comb get(N) Helm semantics** | Product notes CombFilter indexing / ±0.999 clamp differs; upgrade only if measured. | Document get indexing; optional signed feedback clamp policy | Break existing Comb users silently |

### F1 detail (primary)

**File:** `crates/aura-dsp/src/reverb.rs` (`DampedComb` private fields today).

**Done when:**

1. Unit tests: build reverb, process impulse, call `set_decay` / `set_damping`,
   prove next samples change energy / HF content vs frozen controls (finite,
   no alloc in setter).
2. `set_*` never reallocs delay buffers; sample rate / comb lengths unchanged.
3. Non-finite inputs → documented safe clamp or no-op (prefer clamp like `new`).
4. CHANGELOG Unreleased: additive methods, no break.
5. Helmira follow-up (separate product commit): process path applies setters
   when damp/fb params change (or every block from plain values); remove
   init/reset-only honesty comment when live.

**Suggested signature:**

```rust
impl Reverb {
    /// RT-safe. Updates comb feedback from decay in [0, 1] (clamped to 0..0.99).
    pub fn set_decay(&mut self, decay: f32) { /* write all combs_l/r feedback */ }

    /// RT-safe. Updates comb HF damping in [0, 1].
    pub fn set_damping(&mut self, damping: f32) { /* write damp1/damp2 on combs */ }
}
```

Expose `DampedComb::{set_feedback,set_damping}` as crate-private or inherent
methods so FDN/other dens stay consistent if they share the type later.

**Helmira wiring after AURA lands:** path-dep bump → in bus FX pass, if
reverb on, `reverb.set_decay(feedback_param)` + `set_damping(damping_param)`
(plain or smoothed); keep mix as today. Still **no** `Reverb::new` on the
audio thread when knobs move.

---

## B — M4 DSP candidates (evidence only; status after Helmira M4 ship)

Start was existing primitives — **done for M4 product**. Promote only on
measured gap or second consumer.

| # | Candidate | Trigger | Status after Helmira M4/M5 | Non-goal |
|---|-----------|---------|----------------------------|----------|
| B11 | `svf_pass_blend_weights(blend) -> (lp,bp,hp)` | 4a A/B: sum-normalize law wrong vs Helm sqrt weights | Open — listening A/B still pending (UI now exists) | mopo SVF clone |
| B12 | 12/24 cascade helper / driven multi-stage SVF | two SVF + mid tanh insufficient **or** second product | Open — product cascade shipped | baked dual-stage before proof |
| B13 | Shared pre-filter drive helper | many identical `soft_clip_tanh` + non-finite sites | Open — still product call sites | Product dB→lin mapping |
| B14 | `FilterType::BandShelf` | Peak ≠ band-shelf for preset parity | Open — verify on `.helm` import | Fake shelves as Peak |
| B15 | `FormantBank<const N>` + morph | shared x/y bank beyond vowel morph | Open — product A/E/O/I bilinear OK | Helm formant table dump |
| B16 | `TempoDelay` (stereo/sync/ping-pong) | second product needs same law | Open — Helmira delay product-local on `DelayLine` | Opinionated FX bus |
| B17 | **`DcBlocker`** | see **F2** | Promote candidate → F2 | — |
| B18 | Extra `DistortionType` curve | see **F3** | Promote candidate → F3 | Helm type-name enum clone |
| B19 | Stutter / granulator engine | second instrument (Loom-family) | **Stay product-local** (Helmira 96k buffer) | Default framework granulator |
| B20 | Reverb comb/AP network builder | listening + clean generic API | Partially superseded by **F1** live setters; full network builder still optional | Helm tuning table dump |

**Already enough (used in production Helmira):** `StateVariableFilter`,
`blend_svf_outputs`, shelves, `FormantFilter`/`Vowel`, Distortion Soft/Hard/
WaveFold, `DelayLine`, tempo helpers, `Reverb`, `soft_clip_tanh`,
`unison_voice_pan_gains`, `ModRouteTable`+scale, `SmoothRandom`.

Detail: `helmira-driven-gaps.md` §§11–20.

---

## C — CLAP / host (product-driven)

Do when a shipping plugin needs it — not for “CLAP completeness”.

| # | Item | Status | Where |
|---|------|--------|--------|
| C1 | Multi-page `remote-controls` via `ParamInfo.group` | Open | `aura-clap` README Outstanding |
| C2 | Non-zero `latency` + mid-block automation host proof | API exists; needs product plug proof | same |
| C3 | `clap-sys` bump | Only if a **new** extension is required | same |
| C4 | `aura-host` polish (sample-accurate host MIDI, macOS/Linux embed, preset UI) | Done enough for smoke; ponytail leftovers | `aura-host-idea.md` |
| C5 | Per-note expression + ParamMod host proof matrix | Helmira advertises subset; more hosts need CI notes | `aura-clap` README / host tests |
| C6 | State encode defaults / empty Values trap | see **F6** | export + factory preset docs |
| C7 | Tail length reporting guidance | Helmira: 0 dry / `u32::MAX` when delay|reverb cached on | doc recipe only |

Explicit non-goals: AU/AAX/VST2, full CLAP zoo (thread-pool, ambisonic, …).

---

## D — Framework polish (low urgency)

| # | Item | Source | Notes |
|---|------|--------|--------|
| D1 | Split fat `aura-clap` modules | refinement P2 | deferred; no test net; large surface |
| D2 | Extension-trait de-CLAP of `PluginLogic` | refinement P1 honesty leftover | optional before crates.io churn |
| D3 | Optional prose de-slop re-scan | refinement | only if more marketing copy piles up |
| D4 | Extern-tutorial “nicht LX” onboarding | optimisation-todo | after next publish story |
| D5 | Next crates.io publish after 0.14.x cuts | crates-io-prep | when release will exists |
| D6 | Step-seq / arp **note-out** timing contract | step-sequencer-arp doc | Loom-family later; Helmira arp + step = product-local (mod source + mono arp). Do **not** force into AURA for Helmira. |

---

## E — Out of scope (do not “improve” into these)

- Bit-identical Helm / mopo DSP
- Helm unison 15 / harmonize / digital waves 5–10 as framework defaults
- Helm reverb/distortion **name** tables in public API
- Product param IDs, MIDI-note cutoff policy, filter blend UI enums
- Product mono arpeggiator / step phasor (stay Helmira until second law match)
- VST3/LV2 work driven only by Helmira (CLAP-first product)
- Silent host/export forks per product
- `Reverb::new` from UI/automation on the audio thread

---

## Suggested order

```text
1. F1  Reverb set_decay / set_damping     → unblocks Helmira live damp/fb knobs
2. A1  0.14.4 cut (include F1 if ready)
3. F2  DcBlocker (cheap; drop product duplicate when ready)
4. F3  SinFold only after Helmira A/B fails WaveFold
5. F4  noise seed if multi-voice noise diffuse matters in mix
6. F5–F6 / C5–C7 docs+helpers when next CLAP polish slice runs
7. B11–B16 only when listening or .helm import proves gap
8. B19 stutter stays product unless Loom needs same engine
9. D* background hygiene
```

Tick items here when landed; keep one-line CHANGELOG under Unreleased. Spec
surgery still happens in the canonical doc for that topic. After F1 merges,
update `helmira-driven-gaps.md` §20 + Helmira `STATUS.md` honesty line and
product process path in a **separate** Helmira commit.
