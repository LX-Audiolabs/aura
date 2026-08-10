<!-- AGAL:AUTO-START -->
# aura-editor

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura-editor` |
| description | AURA host Editor adapter (Slint on aura-baseview) for CLAP/VST3/LV2 GUI |
| frameworks | aura, aura-baseview, aura-editor, baseview, raw-window-handle, slint |
| generated | `2026-08-10T06:38:20Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=crate id=crates/aura-editor
[ATOM] type=fact | detail=frameworks=aura+aura-baseview+aura-editor+baseview+raw-window-handle+slint
[ATOM] type=fact | detail=roles=entry+manifest+source
[ATOM] type=fact | detail=depends_on=aura-baseview
[ATOM] type=fact | detail=depends_on=aura-core
[ATOM] type=fact | detail=depends_on=aura-params
[ATOM] type=fact | detail=used_by=examples/smoke-gain via depends_on
```

## deps (workspace)
- `aura-baseview`
- `aura-core`
- `aura-params`

## dependents (inbound)
- `examples/smoke-gain` --depends_on--> `aura-editor`

## structure
- public_api symbols: 8 (see json)
- roles: entry, manifest, source

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `crates/aura-editor` in json / `agal --plugin aura-editor .`

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
