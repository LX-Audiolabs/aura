<!-- AGAL:AUTO-START -->
# aura-dsp

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura-dsp` |
| description | AURA DSP — synthesis, effects, analysis, maths (JUCE juce_dsp analogue; LX FX) |
| frameworks | aura |
| generated | `2026-08-17T17:38:48Z` |

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
- … +256 more public symbols

## findings
- [info] **dsp_process_methods**: aura-dsp has 15 methods named process (DSP units, not plugin hooks) · `crates/aura-dsp`

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `crates/aura-dsp` in json / `agal --plugin aura-dsp .`

<!-- AGAL:AUTO-END -->

<!-- AGAL:HUMAN — edit below this line; preserved on regenerate -->

## Intent

_Why this crate/plugin exists. Edit freely._

## Open

- [ ] 

## Decisions

_Architecture choices worth remembering._

## Atoms (human)

_Graph atoms live **above** in AUTO. Add durable decisions/lessons here:_

```text
[ATOM] type=decision|lesson|constraint | detail=…
```
