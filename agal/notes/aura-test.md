<!-- AGAL:AUTO-START -->
# aura-test

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura-test` |
| description | Test helpers for AURA plugins (state round-trip, process smoke) |
| frameworks | aura |
| generated | `2026-08-10T06:38:20Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=crate id=crates/aura-test
[ATOM] type=fact | detail=frameworks=aura
[ATOM] type=fact | detail=roles=entry+manifest
[ATOM] type=fact | detail=has_process=true
[ATOM] type=fact | detail=has_editor=true
[ATOM] type=fact | detail=depends_on=aura-core
[ATOM] type=fact | detail=depends_on=aura-params
```

## deps (workspace)
- `aura-core`
- `aura-params`

## structure
- logic: GainPlug
- params: GainParams (1 fields)
- process: PluginLogic::process @ src/lib.rs
- editor: yes
- process methods (DSP): 1
- public_api symbols: 12 (see json)
- roles: entry, manifest

## findings
- [info] **crate_no_dependents**: aura-test has no inbound workspace edges — unused or only path-included? · `crates/aura-test` · fix: wire `aura-test` as a path dep from a plugin/crate, or remove from workspace

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `crates/aura-test` in json / `agal --plugin aura-test .`

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
