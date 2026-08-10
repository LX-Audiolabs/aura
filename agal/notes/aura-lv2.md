<!-- AGAL:AUTO-START -->
# aura-lv2

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura-lv2` |
| description | AURA LV2 format wrapper (thin, over PluginLogic) |
| frameworks | aura, lv2 |
| generated | `2026-08-10T16:45:08Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=crate id=crates/aura-lv2
[ATOM] type=fact | detail=frameworks=aura+lv2
[ATOM] type=fact | detail=roles=entry+manifest+source
[ATOM] type=fact | detail=depends_on=aura-core
[ATOM] type=fact | detail=depends_on=aura-params
[ATOM] type=fact | detail=used_by=aura via depends_on
```

## deps (workspace)
- `aura-core`
- `aura-params`

## dependents (inbound)
- `aura` --depends_on--> `aura-lv2`

## structure
- public_api symbols: 9 (see json)
- roles: entry, manifest, source

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `crates/aura-lv2` in json / `agal --plugin aura-lv2 .`

<!-- AGAL:AUTO-END -->

<!-- AGAL:HUMAN — edit below this line; preserved on regenerate -->

## Intent

_Why this crate/plugin exists. Edit freely._

## Open

- [x] LV2 UI extension — `lv2ui_descriptor` + `Editor` bridge + `ui:idleInterface`; done 2026-08-10. Host smoke pending a suitable LV2 host on the target platform.

## Decisions

- LV2 UI reuses the same `aura_core::Editor` trait as CLAP/VST3; the wrapper only adds host-bridge (`Lv2Bridge`) and platform widget hand-off.
- UI triples are emitted in TTL only when `PluginLogic::editor()` returns `Some`.

## Atoms (human)

_Graph atoms live **above** in AUTO. Add durable decisions/lessons here:_

```text
[ATOM] type=decision | detail=LV2 UI implemented via shared Editor trait; no separate UI crate needed
[ATOM] type=lesson | detail=Windows REAPER does not ship LV2 support; real host smoke needs Carla/jalv on Linux or a dedicated LV2 host
```
