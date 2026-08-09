<!-- AGAL:AUTO-START -->
# aura

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura` |
| description | AURA — Audio Unified Rust Architecture (umbrella crate) |
| generated | `2026-08-09T10:14:20Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=crate id=crates/aura
[ATOM] type=fact | detail=roles=entry+manifest
[ATOM] type=fact | detail=depends_on=aura-clap
[ATOM] type=fact | detail=depends_on=aura-core
[ATOM] type=fact | detail=depends_on=aura-derive
[ATOM] type=fact | detail=used_by=examples/smoke-gain via depends_on
[ATOM] type=fact | detail=used_by=examples/smoke-synth via depends_on
```

## deps (workspace)
- `aura-clap`
- `aura-core`
- `aura-derive`
- `aura-dsp`
- `aura-lv2`
- `aura-midi`
- `aura-params`
- `aura-vst3`

## dependents (inbound)
- `examples/smoke-gain` --depends_on--> `aura`
- `examples/smoke-synth` --depends_on--> `aura`

## structure
- params: CollidingParams (1 fields)
- params: SubParams (1 fields)
- params: TestParams (5 fields)
- roles: entry, manifest

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `crates/aura` in json / `agal --plugin aura .`

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
