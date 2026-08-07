<!-- AGAL:AUTO-START -->
# aura-params

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura-params` |
| description | AURA parameter system (ranges, smoothers, atomic params) |
| generated | `2026-08-07T17:17:03Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=crate id=crates/aura-params
[ATOM] type=fact | detail=roles=entry+manifest+source
[ATOM] type=fact | detail=used_by=aura via depends_on
[ATOM] type=fact | detail=used_by=aura-clap via depends_on
[ATOM] type=fact | detail=used_by=aura-core via depends_on
```

## dependents (inbound)
- `aura` --depends_on--> `aura-params`
- `aura-clap` --depends_on--> `aura-params`
- `aura-core` --depends_on--> `aura-params`
- `aura-derive` --depends_on--> `aura-params`
- `aura-lv2` --depends_on--> `aura-params`
- `aura-vst3` --depends_on--> `aura-params`

## structure
- public_api symbols: 22 (see json)
- roles: entry, manifest, source

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `crates/aura-params` in json / `agal --plugin aura-params .`

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
