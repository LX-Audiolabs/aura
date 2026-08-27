<!-- AGAL:AUTO-START -->
# aura-preview

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `member` |
| path | `tools/aura-preview` |
| description | Hot-reload preview for AURA plugin .slint UIs (@aura widgets + bundled fonts) |
| frameworks | aura |
| generated | `2026-08-27T06:03:56Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=member id=tools/aura-preview
[ATOM] type=fact | detail=frameworks=aura
[ATOM] type=fact | detail=roles=entry+manifest
[ATOM] type=fact | detail=depends_on=aura-build
```

## deps (workspace)
- `aura-build`

## structure
- roles: entry, manifest

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `tools/aura-preview` in json / `agal --plugin aura-preview .`

<!-- AGAL:AUTO-END -->

<!-- AGAL:HUMAN — edit below this line; preserved on regenerate -->

## Intent

Slint interpreter preview without compiling the plugin. `cargo aura preview`
or `cargo run -p aura-preview -- path/to/ui/main.slint`.

## Open

- None. Live reload uses `notify` here; `cargo aura watch` stays mtime-poll
  and dep-free.

## Decisions

- Preview is UI-only — no `PluginLogic` / audio.

## Atoms (human)

```text
[ATOM] type=decision | detail=aura-preview = Slint interpreter; watch (rebuild+install) is cargo-aura and does not use this crate
```
