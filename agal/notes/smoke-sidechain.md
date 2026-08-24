<!-- AGAL:AUTO-START -->
# smoke-sidechain

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `member` |
| path | `examples/smoke-sidechain` |
| description | AURA in-tree CLAP smoke — stereo FX with mono sidechain |
| frameworks | aura |
| generated | `2026-08-24T16:17:07Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=member id=examples/smoke-sidechain
[ATOM] type=fact | detail=frameworks=aura
[ATOM] type=fact | detail=roles=entry+manifest
[ATOM] type=fact | detail=has_process=true
[ATOM] type=fact | detail=depends_on=aura
```

## deps (workspace)
- `aura`

## structure
- logic: SmokeSidechain
- params: SidechainParams (1 fields)
- process: PluginLogic::process @ src/lib.rs
- roles: entry, manifest

## api surface
- `struct DspState` · `src/lib.rs`
- `struct SidechainParams { amount: FloatParam }` · `src/lib.rs`
- `struct SmokeSidechain` · `src/lib.rs`
- `impl PluginLogic for SmokeSidechain` · `src/lib.rs`

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `examples/smoke-sidechain` in json / `agal --plugin smoke-sidechain .`

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
