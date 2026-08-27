<!-- AGAL:AUTO-START -->
# aura-dsp

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura-dsp` |
| description | AURA DSP — synthesis, effects, analysis, maths (JUCE juce_dsp analogue; LX FX) |
| frameworks | aura |
| generated | `2026-08-27T17:47:49Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=crate id=crates/aura-dsp
[ATOM] type=fact | detail=frameworks=aura
[ATOM] type=fact | detail=roles=entry+manifest+source+state
[ATOM] type=fact | detail=has_process=true
[ATOM] type=fact | detail=depends_on=aura-params
[ATOM] type=fact | detail=used_by=aura via depends_on
```

## deps (workspace)
- `aura-params`

## dependents (inbound)
- `aura` --depends_on--> `aura-dsp`

## structure
- process methods (DSP): 15
- public_api symbols: 80 (see json)
- roles: entry, manifest, source, state

## api surface
- `trait Filter` · `src/fx/mod.rs`
- `trait ModulationSource` · `src/modulation.rs`
- `struct AmbisonicsEncoder { … }` · `src/acoustics/ambisonics.rs`
- `struct BFormatSample { w: f32, x: f32, y: f32, z: f32 }` · `src/acoustics/ambisonics.rs`
- … +261 more public symbols

## findings
- [info] **dsp_process_methods**: aura-dsp has 15 methods named process (DSP units, not plugin hooks) · `crates/aura-dsp`

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `crates/aura-dsp` in json / `agal --plugin aura-dsp .`

<!-- AGAL:AUTO-END -->

<!-- AGAL:HUMAN — edit below this line; preserved on regenerate -->

## Intent

Portable DSP (JUCE `juce_dsp` analogue): filters, delay lines, smoothing helpers,
math. Product FX/algos that are not host-infra land here (`docs/dsp-layout.md`).
Messages / UMP stay in `aura-midi`.

## Open

- Real-synth smoothing / expression→knob matrix is plugin DSP, not a wrapper hole.

## Decisions

- Allocate delay lines / voices in `prepare`/`init`/`reset`, never in `process`.
- After sample-rate change, recompute coeffs **and** clear filter state.

## Atoms (human)

```text
[ATOM] type=decision | detail=Portable DSP algos land under aura-dsp modules (docs/dsp-layout.md)
[ATOM] type=lesson | detail=SVF bei q=0 gibt NaN — Guard in prepare() noetig
[ATOM] type=lesson | detail=BiquadFilter state variables z1/z2 müssen in prepare() auf 0.0 resettet werden nach SR-Change
```
