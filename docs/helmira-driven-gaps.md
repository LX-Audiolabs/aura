# AURA gaps driven by Helmira (Milestone 3-4+)

Stand: **2026-11-09** · AURA workspace **0.14.3** + **Unreleased** · product: sibling `../Helmira`
Helmira `main` @ `996c863` (M3 complete, 3f step-seq merged). Next product = **M4 filter/FX**.

Evidence-based backlog. Helmira is the product; AURA only gets primitives that
are generic, RT-safe, and measured. No silent host/export/editor fork. No “just
in case” API.

**Not this file:** Helm-specific unison 15 / harmonize, Helm digital wavetables
5–10, PD/VPS synth, Arp/Step-Seq product UI, Helmira CLAP param IDs.

Related: [`refinement-backlog.md`](./refinement-backlog.md),
[`framework-improvements.md`](./framework-improvements.md) (priority roll-up),
[`dsp-synth-roadmap.md`](./dsp-synth-roadmap.md),
[`versioning.md`](./versioning.md). Helmira local roadmap (gitignored there):
`docs/planning/helmira-roadmap.md` M3.

---

## Policy (same as Helmira AGENTS)

1. Gap → **failing test in the consumer** (Helmira) or a minimal AURA unit test
   that states the contract. Then AURA fix. No silent product fork of host path.
2. Prefer **promote after prototype**: if only Helmira needs it, keep it in
   Helmira until a second product or A/B wins.
3. SemVer: additive APIs = minor (0.14.x still path-dep friendly); breaking
   `ModMatrix` enum surface = document + prefer new type alongside old.
4. RT: no alloc / lock / file / graph rebuild on audio thread after init/reset.

---

## Already OK for M3 start (no AURA work)

| Piece | Where | Note |
| --- | --- | --- |
| `Adsr` / `Ahdsr` / `MultiStageEnvelope` | `aura-dsp` envelope | Amp/mod/filter envs; start with `Adsr` |
| `Lfo` basic shapes + bi/uni + depth | `modulation.rs` | freerun Hz OK |
| `Transport` (tempo, beats, …) | `aura-core` + CLAP map | read in `ProcessContext` |
| `Oscillator::sample_offset` | 0.14.3 | Helmira cross-mod already uses it |
| `ParamSmoother` | smoothing | automation ramps |
| SVF / Biquad / Moog | filter | **M4** main work; M3 may stub cutoff targets only |
| Note expressions / note_id | clap/core | already exercised in Helmira 1a |

---

## Do **not** expand these into Helm clones

| Existing | Why wrong for Helm | Action |
| --- | --- | --- |
| `mod_matrix::ModMatrix` | Fixed 8 src × 8 dst enums, 16 routes; toy synth surface | Leave or deprecate later. **Do not** add Helm string names / 100 destinations |
| `UnisonOscillator` | max 8, `1/N`, linear cents, stereo spread, no harmonize / per-voice CM | Helmira keeps own extras; optional later pure spread helper only |
| Effect-internal LFOs | chorus/flanger owned oscs | not a product LFO primitive |

---

## Priority gaps (work here)

### P0 — unblocks Helmira M3 tempo-sync / poly LFO

#### 1. Tempo / beat helpers (dsp or small `aura_dsp::tempo` / core util)

**Need:** `bpm + division_index → hz`, optional `position_beats → phase` for
synced LFO/seq. Helm free-run uses exponential seconds; sync uses fixed
division table — product maps indices; AURA supplies **math**, not Helm tables.

**API sketch (final names free):**

```rust
fn hz_from_tempo(bpm: f32, beats_per_cycle: f32) -> f32;
fn phase_from_beats(position_beats: f64, beats_per_cycle: f64) -> f32; // 0..1
```

**Done when:** unit tests at 120 BPM / 1 beat = 2 Hz; non-finite BPM → safe
fallback documented; no alloc.

**Helmira use:** mono/poly LFO sync, step-seq rate, later delay tempo (M4).

#### 2. `Lfo` phase control

**Need:** `reset_phase()`, `set_phase(f32)`, maybe `phase() -> f32` (ro).
Retrigger on note without reconstructing LFO. Gate-retrigger is product policy.

**Done when:** tests prove phase snap + continued `next_value` continuity of
freq; S&H does not require extra product RNG reset unless documented.

#### 3. (Optional same PR) freerun frequency already OK — document max useful
range and `set_frequency(0)` behavior for stopped clocks.

---

### P1 — real framework hole (after Helmira N-slot prototype)

#### 4. RT route table (replace / supersede toy `ModMatrix`)

**Problem:** product matrices are **N preallocated slots**: `(src_id, dst_id,
depth, enabled)` with **u16/u8 indices** (or stable param ids), not closed synth
enums. Helm: many named sources/dests, mono+poly scope, amount can itself be
modulated, preset import lists arbitrary pairs.

**Direction:** new type, e.g. `ModRouteTable<const N: usize>` or fixed
`[ModRoute; N]` with:

- `src: u16`, `dst: u16`, `depth: f32`, `enabled: bool` (and later optional
  `scale_src: u16`)
- `clear_outputs()`, `accumulate(src_values: &[f32], out: &mut [f32])` — no
  heap, no enum match on hot path
- product owns **name → id** maps and mono vs poly buffer sets

**Explicit non-goals for v1:** string graph, dynamic connect on audio thread,
audio-rate per-route oversampling, Helm `ValueSwitch` graph.

**Done when:**

- AURA tests: N routes sum depths, disabled skip, depth clamp, fill-full add fails
  or saturates documented
- Helmira can map ≥ dual env + 2 mono LFO + poly LFO + random → pitch/amp/cutoff
  stubs without forking AURA host
- old `ModMatrix` either thin wrapper or docs “legacy example only”

**Order:** Helmira builds **local N-slot first** (learn sizes/IDs). Promote
generic core into AURA when the table shape stabilizes — copy types up, don’t
redesign twice.

#### 5. Secondary scale / bipolar depth (with route table v2) — done

**Status:** done 2026-11-08; merged to `main` (PR #18 + docs follow-up).
Local plan (gitignored `docs/` tree may hide it):
`docs/planning/2026-11-08-mod-route-scale.md`. **This section is the tracked source of truth.**

Helm `amount * modulation_scale`. Helmira 3b evidence: poly LFO → pitch via
`ModRouteTable`; product uses **semitones** (`hz *= 2^(out/12)`). `ModRoute::new`
clamps `depth` to **[-1, 1]** → one route max **±1 st**. `route_mut` can poke depth
past ±1 (accumulate only checks finite) — API inconsistent.

**Implementer:** open AURA, branch e.g. `feature/mod-route-scale`. TDD. Minimal
diff. No Helm names in public API. **No Helmira edits in this PR** (consumer after merge).

##### Locked design

```rust
pub struct ModRoute {
    pub src: u16,
    pub dst: u16,
    /// Normalized amount. Constructors clamp to [-1.0, 1.0]; non-finite → 0.0.
    pub depth: f32,
    /// Destination range scale in **product units** (st, gain, cutoff, …).
    /// Default **1.0**. Any finite OK. Non-finite → **0.0**.
    pub scale: f32,
    pub enabled: bool,
}
```

| API | Behavior |
| --- | --- |
| `ModRoute::new(src, dst, depth)` | Keep signature. `scale = 1.0`. depth ±1 clamp / non-finite→0. enabled true. |
| `ModRoute::new_scaled(src, dst, depth, scale)` | depth ±1 clamp; scale finite else 0; enabled true. |

**`accumulate`:** `out[dst] += src * depth * scale` when enabled and all finite and ids in range. Zero `out` first. Skip non-finite depth/scale/src. No alloc, no panic.

**Serde:** serialize `scale`; missing field on deserialize → **1.0** (old JSON identity).

**Non-goals:** vibrato/pitch types, `scale_src`, mono/poly bit, Hz math in AURA, `ModMatrix` enum churn, Helmira consumer PR.

**Product meaning of `scale` is product-defined.** Doc example only:

```text
# after AURA merge, in Helmira (separate PR):
ModRoute::new_scaled(SRC_POLY_LFO, DST_PITCH, amount_ui, 12.0)
# product: hz *= 2^(out_pitch/12)
ModRoute::new(SRC_POLY_LFO, DST_AMP, amount)  # scale default 1
# product: mono *= (1 + out_amp).max(0)
```

##### Files

- `crates/aura-dsp/src/mod_route_table.rs` — type, ctors, accumulate, tests
- this file — mark done when landed
- `CHANGELOG.md` — Unreleased

##### Task 1 — failing tests

1. `new` clamps depth ±1; **`scale == 1.0`**
2. `new_scaled(0, 0, 1.0, 12.0)` → depth 1, scale 12
3. non-finite scale → 0; non-finite depth → 0; depth 2 → clamp 1, scale kept
4. accumulate src 1, depth 1, scale 12 → out 12
5. src 0.5, depth 0.5, scale 8 → out 2.0
6. scale NaN on route → skipped
7. serde round-trip keeps scale; JSON **without** `scale` → 1.0
8. fix struct-literal tests for new field

```bash
cargo test -p lx-aura-dsp mod_route -- --nocapture   # red
```

##### Task 2 — implement

Add field (table zero-slots `scale: 1.0`), ctors, accumulate multiply, serde default, rustdoc (depth=amount, scale=product unit).

```bash
cargo test -p lx-aura-dsp mod_route -- --nocapture
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

##### Task 3 — CHANGELOG + mark done

```markdown
### Added
- `ModRoute::scale` + `ModRoute::new_scaled`; `accumulate` = `src * depth * scale`.
  Missing serde `scale` defaults to 1.0. Depth stays ±1 amount.
```

Check this §5 off / “done” line + date. Commit example:

```bash
git add crates/aura-dsp/src/mod_route_table.rs docs/helmira-driven-gaps.md CHANGELOG.md
git commit -m "feat(dsp): per-route ModRoute scale for dest ranges"
```

##### Review focus

Depth still ±1 on ctors; scale unrestricted finite; old JSON identity; no NaN out; no product names; RT-safe.

##### After AURA merge (Helmira separate)

Path-dep picks API. `new_scaled(..., amount, 12.0)` for pitch. Drop ±1 st shortcut comment. Optional multi-st pitch test.


---

### P2 — nice for M3 quality / M4

#### 6. Envelope delay-before-attack

Only if measured Helm amp/mod/filter envelopes need silent delay stage beyond
`Ahdsr` hold. Prefer documenting `MultiStageEnvelope` recipe over new type.

#### 7. SVF multi-out blend + drive (M4) — **partially done**

`blend_svf_outputs` shipped (normalized). Drive + 12/24 + shelf + Helm weight
law: see **M4 sections 11-14**. Still: compare impulse/A/B; extend only on fail.
No "copy mopo SVF" dump.

#### 8. Unison stereo spread helper — **done** (`unison_voice_pan_gains`)

Product may wire later as **feature beyond Helm**, default off. No further AURA
work unless API gap appears.

#### 9. `RandomSource` / smooth random — **done** (`SmoothRandom`)

Helmira 3e uses it. Note-on discrete random stays product. Further RNG laws
only if second product needs them.

#### 10. Step sequencer / arp clock — **product-done for mod source**

Helmira 3f = mono step **mod source** (local phasor). Stay product-side for
note-out arp / pattern engines. Shared clock = existing tempo helpers only.
See `docs/step-sequencer-arp-clap-timing.md`.

---

## Suggested AURA work order (historical M3 — done)

```text
A. Tempo/beat helpers + Lfo phase reset     → done
B. (Helmira) N-slot route table local         → done → promoted
C. Promote ModRouteTable into aura-dsp        → done
D. M4 filter A/B → see M4 section below
E. spread helper / SmoothRandom               → done
```

---

## Checklist (tick when merged)

### M3 framework
- [x] P0 tempo/beat helpers + tests
- [x] P0 `Lfo::{reset_phase,set_phase,phase}` + tests
- [x] P1 `ModRouteTable` (or equivalent) + tests; docs vs legacy `ModMatrix`
- [x] P1 route scale + serde compatibility
- [x] P2 `blend_svf_outputs` + tests
- [x] P2 `unison_voice_pan_gains` + tests
- [x] P2 `SmoothRandom` + tests
- [x] CHANGELOG Unreleased lines on each land
- [ ] Helmira path-dep lock picks up next AURA cut (0.14.4+) when tagged; no host fork

### M4 framework (evidence only)
- [ ] section 11 SVF blend weight helper (only if 4a A/B fails normalize law)
- [ ] section 12 multi-stage SVF helper (only if cascade repeats)
- [ ] section 13 shared pre-filter drive (only if repeated)
- [ ] section 14 `BandShelf` if Biquad gap proven in 4b
- [ ] section 15 generic formant bank (only if x/y morph shared)
- [ ] section 16 tempo stereo delay primitive (only if second product)
- [x] section 17 `DcBlocker` (implemented in `aura-dsp`; `R` default 0.995)
- [ ] section 18 extra `DistortionType` (only after A/B)
- [ ] section 19 stutter engine (default stay product)
- [ ] section 20 reverb parity (default accept AURA reverb)

---

## M3 -> done in AURA (promote checklist)

| Item | Status | Notes |
| --- | --- | --- |
| P0 tempo/beat helpers | **done** | `hz_from_tempo`, `phase_from_beats`; used by mono/poly LFO + step-seq; **reuse for M4 delay/stutter tempo** |
| P0 `Lfo` phase API | **done** | `phase` / `set_phase` / `reset_phase` |
| P1 `ModRouteTable` | **done** | product uses table; toy `ModMatrix` stays legacy |
| P1 `ModRoute::scale` / `new_scaled` | **done** | `new_scaled` + serde default scale 1.0; PR #18 |
| P2 `blend_svf_outputs` | **done** | normalized LP/BP/HP; see M4 section 11 if Helm weight law differs |
| P2 `unison_voice_pan_gains` | **done** | product extra spread still default-off later |
| P2 `SmoothRandom` | **done** | Helmira 3e wired |
| P2 step-seq / arp clock | **product-local** | Helmira 3f owns phasor; do **not** add AURA StepGenerator unless second product needs same law |

Helmira M3 slice order (historical):

```text
3a  mod + filter envs (Adsr)
3b  poly LFO freerun + note-on reset
3c  mono LFO + transport sync
3d  16 user route slots
3e  random + SmoothRandom + Tier-A dests
3f  step-seq mod source  -> MERGED
```

---

## M4 -- filter + FX (new; evidence-first)

Helmira roadmap Milestone 4: per-voice filter blends/shelves/formant/stutter, then
global **Distortion -> Delay -> DC-Blocker -> Reverb -> Master**.

**Product slice plan (Helmira STATUS):** 4a SVF core -> 4b drive/style/shelf ->
4c formant -> 4d distortion -> 4e delay -> 4f reverb/DC/master.

**Policy unchanged:** start on existing AURA primitives. Gap -> failing Helmira
(or minimal AURA) test -> fix. No "copy mopo" dump. No Helm CLAP IDs / param
names in public AURA API.

### Already OK for M4 start (no AURA work before 4a)

| Piece | Where | Helmira use |
| --- | --- | --- |
| `StateVariableFilter` + `SvfOutput` | `filter` | 4a multi-out SVF |
| `blend_svf_outputs` | `filter` | 4a LP/BP/HP mix (normalized) |
| `BiquadFilter` (`LowShelf`/`HighShelf`/`Peak`/...) | `filter` | 4b shelf path; 4c formant BP |
| `MoogLadder` / predictive ladder | `filter` | **not** Helm main path; optional alt later |
| `soft_clip_tanh` | `dsp_util` | 4b drive pre-filter |
| `FormantFilter` / `FormantSynth` + `Vowel` morph | `synth/formant` | 4c start; may keep Helm x/y tables product-side |
| `Distortion` Soft/Hard/WaveFold | `effects` | 4d bus |
| `DelayLine` / `CombFilter` / `AllpassDelay` | `delay` | 4e delay; feedback already product-local != Comb |
| Tempo helpers | `tempo` | 4e delay sync; stutter rate if built |
| `Reverb` / `FdnReverb` / `RoomReverb` | reverb/acoustics | 4f; timbre != Helm comb OK |
| `Adsr` | envelope | `fil_*` already in product; wire depth in 4a |

### Do not put in AURA (stay product / out of scope)

| Helm / Helmira thing | Why stay out |
| --- | --- |
| MIDI-note cutoff 28..127 + keytrack + `fil_env_depth` scaling | product pitch/MIDI policy |
| Helm `filter_blend` 0..2 UI + style enum as framework enum | product params |
| 15-voice unison / harmonize / digital waves 5-10 | already decided |
| Step-seq note-out / arp patterns | Loom-family later; 3f is mod source only |
| Stutter buffer engine | product-local (like step phasor) until second user |
| Helm reverb comb tuning tables / distortion type names | A/B first; map onto existing types |
| Bit-identical mopo SVF coefficients | forbidden forever |

---

### M4 candidate promotions (only after measured gap)

#### 11. SVF blend weight law (4a A/B)

**Have:** `blend_svf_outputs` -- clamps weights >=0, **sum-normalizes**.

**Helm:** `filter_blend` 0..2 ->
`lp=sqrt(clamp(1-b))`, `bp=sqrt(clamp(1-|b-1|))`, `hp=sqrt(clamp(b-1))`,
applied as TPT **m0/m1/m2** mix **without** normalizing to unit sum; 12/24 dB
styles; drive via `quickTanh`.

**Start in product:** map blend -> three weights, call `blend_svf_outputs` **or**
raw `lp*w_l+...` without normalize if that matches listening better.

**Promote only if** A/B fails and a second product wants same law:

```rust
/// Pure weights for continuous LP<->BP<->HP blend in [0, 2]. No normalize.
fn svf_pass_blend_weights(blend: f32) -> (f32, f32, f32);
```

**Non-goal:** replace `StateVariableFilter` with mopo clone; dual-stage baked
type before product proves two cascaded `StateVariableFilter`s insufficient.

#### 12. 12/24 dB cascade helper (4b)

Helm 24 dB = two SVF stages, `tanh` between, resonance `sqrt` for cascade.

**Start in product:** two `StateVariableFilter` + `soft_clip_tanh` mid.

**Promote if** generic "driven multi-stage SVF" repeats (second synth): thin
helper or `CascadedSvf { stages: 1|2 }` with explicit drive hook. Keep state
external or document reset rules.

#### 13. Filter input drive (4b)

Helm scales input by drive then `quickTanh` (12 dB path).

**Start:** `soft_clip_tanh(x, drive_lin)` in product (drive from dB param).

**Promote if** many call sites want identical pre-filter saturator + non-finite
guards; else leave `dsp_util`.

#### 14. Band / multi shelf completeness (4b)

**Have:** `FilterType::LowShelf`, `HighShelf`, `Peak`.

**Helm shelves:** low / band / high shelf modes on filter style path.

**Gap check:** if band-shelf (not peak) needed for preset parity and Biquad
math is generic -> add `FilterType::BandShelf` + coeffs + tests.

**No work until** 4b hits missing type in product compile/test.

#### 15. Formant bank morph beyond vowels (4c)

**Have:** `FormantFilter`, `FormantSynth::morph(vowel_a, vowel_b, t)`.

**Helm:** `FormantManager` = N parallel gained band-passes; **x/y** pad morph
across a 2D table (not only AEOIU).

**Start:** product owns x/y -> formant freq/gain/Q tables; can still use
`BiquadFilter` BP or `FormantFilter::new`.

**Promote if** generic `FormantBank<const N>` + `set_formant(i, f, q, gain)` +
optional bilinear morph becomes shared (vocoder-adjacent products).

#### 16. Stereo tempo delay primitive (4e)

**Have:** `DelayLine`, tempo hz helpers; no opinionated ping-pong/sync delay FX.

**Helm:** delay time freerun exp secs **or** tempo division; feedback +/-; dry/wet;
on switch.

**Start:** product wrapper (mirror mono LFO sync mapping).

**Promote if** second product needs same: e.g. `TempoDelay` with
`set_time_samples` / `set_time_beats`, stereo ping-pong flag, feedback clamp,
RT-safe resize policy (**max len at init only**).

#### 17. DC blocker (4f)

**Have:** HP biquad / butterworth helpers in `fx`; `aura_dsp::dc_blocker::DcBlocker`.

**Helm:** dedicated DC filter before master.

**Start:** 1-pole HP (~5-20 Hz) or biquad HP in product.

**Done:** `DcBlocker::new()` / `with_r(r)` / `process(x)` with default
`R = 0.995`, non-finite input mapped to zero, and denormal flushing. Product
may retain exact-zero snap for dry-silence policy.

#### 18. Distortion type set (4d)

**Have:** SoftClip / HardClip / WaveFold.

**Helm:** four product types (names/curves differ).

**Start:** map best-effort onto existing three + dB->lin drive + mix.

**Promote:** new `DistortionType` variant **only** when A/B shows missing curve
and algorithm is generic (document transfer curve in test).

#### 19. Stutter granulator (optional late M4 / later)

Helm mono stutter: circular buffer, stutter period, resample countdown, edge
softness.

**Stay product-local** (memory size policy is product/host). AURA only if Loom
or second instrument needs same engine -- then fixed max-seconds at `new`, no
audio-thread alloc.

#### 20. Reverb algorithm parity (4f)

**Have:** several reverbs. Helm = comb + allpass tuning tables.

**Action:** use AURA reverb; accept timbre delta. Port Helm tables **into
Helmira** only if listening demands; promote generic comb/AP network builder
only with clean API + tests, not raw table dump.

---

### Suggested AURA work order (M4 era)

```text
0. ~~Merge feature/mod-route-scale~~ done; cut 0.14.4 when ready
1. Helmira 4a on current SVF + blend          -> no AURA PR
2. After 4a A/B: optional section 11 weights  -> small pure fn
3. Helmira 4b cascade/drive/shelf in product  -> 12-14 only if stuck
4. 4c formant tables product                  -> 15 only if shared morph needed
5. 4d-4f on existing FX                       -> 16-18 after evidence
6. Optional section 17 DcBlocker anytime (cheap win)
```

Helmira M4 slice order (product, not this repo):

```text
4a  SVF on/cutoff/res/keytrack/fil_env_depth/blend + route dest 2 live
4b  drive + 12/24 dB style + shelf
4c  formant
4d  distortion bus
4e  delay tempo/sync
4f  reverb + DC + master path
```

---

## Future — Helmira filter-type expansion (flag, not scheduled)

**Product ask (2026-11-10):** after Helm-parity SVF UI/layout settles, Helmira wants a **filter type menu** beyond current style 12/24/shelf:

| Type | AURA status | Notes |
| --- | --- | --- |
| **Moog Ladder** | **Already in `aura-dsp`** (`filter::MoogLadder`, also `PredictiveLadder`) | Helmira does **not** wire it yet — product param/UI slice later (IDs ≥ 185). Prefer existing primitive over a second ladder. |
| **Korg MS-20** character | Missing | New `aura-dsp` filter candidate when slice starts |
| **Steiner-Parker** | Missing | New `aura-dsp` filter candidate |
| **Oberheim SEM** multimode character | Partial overlap with SVF blend | Decide: dedicated type vs document “use SVF blend”; if distinct timbre needed → new primitive |

**Policy:** implement specialty filters in **`aura-dsp` first** (generic, RT-safe, unit-tested). Helmira only adds CLAP params + UI. No remap of Helmira IDs 142–149. Product-local only if a model stays Helm-odd after A/B, then promote.

Helmira mirror note (gitignored there): `docs/superpowers/specs/future-filter-types-note.md`.  
Roadmap overlap: [`dsp-synth-roadmap.md`](./dsp-synth-roadmap.md) Stage 1 (Moog already listed; add MS-20 / Steiner / SEM there).

---

## Out of scope forever (unless new product brief)

- Bit-identical Helm DSP
- Wrapping Helm C++/JUCE binary
- Helm factory wavetable dump “just in case”
- VST3/LV2 work driven only by Helmira (CLAP-first product)

Source of requirements: Helmira sessions 2026-11-07..2026-11-09 (M2 osc chain,
M3 mod matrix/LFO/random/step-seq through PR #14 @ `996c863`); AURA path-dep
0.14.3 + Unreleased (`sample_offset`, tempo, Lfo phase, ModRouteTable, blend,
SmoothRandom, pan gains). Helm pin `abdedd527e6e1cf86636f0f1e8a3e75b06ed166a`
(`mopo/state_variable_filter`, formant_manager, stutter, delay, reverb, distortion).
