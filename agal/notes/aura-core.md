<!-- AGAL:AUTO-START -->
# aura-core

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura-core` |
| description | AURA core: process, editor, plugin info (minimal surface) |
| frameworks | aura |
| generated | `2026-08-26T11:53:59Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=crate id=crates/aura-core
[ATOM] type=fact | detail=frameworks=aura
[ATOM] type=fact | detail=roles=audio+entry+manifest+source+state+ui
[ATOM] type=fact | detail=depends_on=aura-midi
[ATOM] type=fact | detail=depends_on=aura-params
[ATOM] type=fact | detail=used_by=aura via depends_on
[ATOM] type=fact | detail=used_by=aura-clap via depends_on
[ATOM] type=fact | detail=used_by=aura-editor via depends_on
```

## deps (workspace)
- `aura-midi`
- `aura-params`

## dependents (inbound)
- `aura` --depends_on--> `aura-core`
- `aura-clap` --depends_on--> `aura-core`
- `aura-editor` --depends_on--> `aura-core`
- `aura-lv2` --depends_on--> `aura-core`
- `aura-test` --depends_on--> `aura-core`
- `aura-vst3` --depends_on--> `aura-core`

## structure
- params: TwoParams (0 fields)
- public_api symbols: 75 (see json)
- roles: audio, entry, manifest, source, state, ui

## api surface
- `trait Editor` · `src/editor.rs`
- `trait EditorBridge` · `src/editor.rs`
- `trait IntoEditor` · `src/editor.rs`
- `trait PluginLogic` · `src/plugin.rs`
- … +71 more public symbols

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `crates/aura-core` in json / `agal --plugin aura-core .`

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
