# AURA gaps driven by Helmira (Milestone 3+)

Stand: 2026-11-07 · AURA workspace **0.14.3** · product: sibling `../Helmira`

Evidence-based backlog. Helmira is the product; AURA only gets primitives that
are generic, RT-safe, and measured. No silent host/export/editor fork. No “just
in case” API.

**Not this file:** Helm-specific unison 15 / harmonize, Helm digital wavetables
5–10, PD/VPS synth, Arp/Step-Seq product UI, Helmira CLAP param IDs.

Related: [`refinement-backlog.md`](./refinement-backlog.md),
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

#### 5. Secondary scale / bipolar depth (with route table v2)

Helm `amount * modulation_scale`. Generic: optional second multiplier per route
or `depth = amount * scale` prepared once per block by product. Prefer product
pre-multiply until two products need it in-framework.

---

### P2 — nice for M3 quality / M4

#### 6. Envelope delay-before-attack

Only if measured Helm amp/mod/filter envelopes need silent delay stage beyond
`Ahdsr` hold. Prefer documenting `MultiStageEnvelope` recipe over new type.

#### 7. SVF multi-out blend + drive (M4)

Helm driven 12/24 dB SVF low/band/high blend + shelf path. AURA has
`StateVariableFilter` — compare impulse/A/B; extend only on fail. No “copy
mopo SVF” dump.

#### 8. Unison stereo spread helper (product extra, post-M3)

Pure fn or thin helper: per-voice pan gain from index/sign. **Not** baking
spread into Helm-compat unison defaults. Helmira wants this later as **feature
beyond Helm**, default off.

#### 9. `RandomSource` / smooth random

Helm poly random ≠ LFO S&H only. Prototype in Helmira; promote if second
product wants same law.

#### 10. Step sequencer / arp clock

Stay **product-side** (Helmira / later Loom-family). AURA may later expose only
shared clock/phasor (see P0 helpers). See also local note
`docs/step-sequencer-arp-clap-timing.md` if still accurate.

---

## Suggested AURA work order

```text
A. Tempo/beat helpers + Lfo phase reset     → small 0.14.x patch
B. (Helmira) N-slot route table local         → product learns N, src/dst ids
C. Promote ModRouteTable into aura-dsp        → deprecate toy enums over time
D. M4 filter A/B → SVF blend only if needed
E. Optional spread helper / random when products ask
```

Helmira M3 slice order (product, not this repo):

```text
3a  mod + filter envs (Adsr)
3b  poly LFO freerun → hard dest or N-slot v0
3c  mono LFO + transport sync (needs A)
3d  full N-slot routes + more destinations
3e  random / step-seq (local)
3f  cutoff stub → M4 filter
```

---

## Checklist (tick when merged)

- [x] P0 tempo/beat helpers + tests
- [x] P0 `Lfo::{reset_phase,set_phase,phase}` + tests
- [x] P1 `ModRouteTable` (or equivalent) + tests; docs vs legacy `ModMatrix`
- [x] CHANGELOG Unreleased lines on each land
- [ ] Helmira path-dep lock picks up version; no host fork

---

## Out of scope forever (unless new product brief)

- Bit-identical Helm DSP
- Wrapping Helm C++/JUCE binary
- Helm factory wavetable dump “just in case”
- VST3/LV2 work driven only by Helmira (CLAP-first product)

Source of requirements: Helmira session 2026-11-07 after oscillator chain 1a–1f
(PR path) + `sample_offset` 0.14.3; Helm pin analysis `abdedd527e6e1cf86636f0f1e8a3e75b06ed166a`.
