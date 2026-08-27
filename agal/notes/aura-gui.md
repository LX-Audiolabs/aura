<!-- AGAL:AUTO-START -->
# aura-gui

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `member` |
| path | `tools/aura-gui` |
| description | Visual AURA project console — thin Slint shell over `cargo aura` CLI |
| frameworks | aura, slint |
| generated | `2026-08-27T06:03:56Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=member id=tools/aura-gui
[ATOM] type=fact | detail=frameworks=aura+slint
[ATOM] type=fact | detail=roles=build+entry+manifest+slint
[ATOM] type=fact | detail=depends_on=aura-build
```

## deps (workspace)
- `aura-build`

## structure
- roles: build, entry, manifest, slint

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `tools/aura-gui` in json / `agal --plugin aura-gui .`

<!-- AGAL:AUTO-END -->

<!-- AGAL:HUMAN — edit below this line; preserved on regenerate -->

## Intent

Optional GUI sugar over `cargo aura` paths. Not the authoring surface —
Cheatsheet + CLI are. Long-term host loop is `aura-host`, not this crate.

## Open

- Deferred while `aura-host` is the host-side loop.

## Decisions

- No extra GUI overhead for the framework author path.

## Atoms (human)

```text
[ATOM] type=decision | detail=aura-gui zurückgestellt — Cheatsheet reicht als primäre Developer-UX; kein GUI-Overhead bis aura-host konkreter ist
```
