<!-- AGAL:AUTO-START -->
# aura-gui

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `member` |
| path | `tools/aura-gui` |
| description | Visual AURA project console — thin Slint shell over `cargo aura` CLI |
| frameworks | aura, slint |
| generated | `2026-08-27T16:56:27Z` |

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

Legacy project-console tool under `tools/aura-gui`. **Not** a workspace
member anymore; `cargo aura gui` was removed. Dev loop is
`cargo aura run` → `aura-host`. Tree may still exist on disk — do not wire
it back without an explicit product ask.

## Open

- [x] Superseded by `aura-host` / `cargo aura run` (2026-08).

## Decisions

- CLI + Cheatsheet remain the authoring surface; no second GUI console in-tree.

## Atoms (human)

```text
[ATOM] type=decision | detail=aura-gui superseded — cargo aura run / aura-host is the host-side loop; tools/aura-gui not a workspace member
```
