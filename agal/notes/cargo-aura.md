<!-- AGAL:AUTO-START -->
# cargo-aura

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `member` |
| path | `tools/cargo-aura` |
| description | Build tool for AURA audio plugins — cargo aura new|build|install|doctor |
| generated | `2026-08-10T16:45:08Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=member id=tools/cargo-aura
[ATOM] type=fact | detail=roles=entry+manifest+source
```

## structure
- roles: entry, manifest, source

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `tools/cargo-aura` in json / `agal --plugin cargo-aura .`

<!-- AGAL:AUTO-END -->

<!-- AGAL:HUMAN — edit below this line; preserved on regenerate -->

## Intent

Author CLI: scaffold, build, install, doctor. GUI (`aura-gui`) is sugar over
the same paths. Orientation mesh is `cargo aura mesh` → `agal` (optional).

## Open

- [ ] In-host binary swap (Windows file lock) — host must unload today
- [ ] `aura-gui` identity pass (Stage 6 UI, later)

## Decisions

- `watch` polls mtimes with std only (no `notify` dep). `preview` already uses `notify` for Slint.
- Default watch format is `--clap`. `--no-install` builds without copying.
- `--hot` installs `aura-hot` as `Name.clap` and the real plugin as `Name.impl.*`. Host never maps the impl, so watch can overwrite it. Re-add instance to run new DSP.
- `mesh` is not a doctor gate. Builds work without agal.
- Install copy retries when the host still maps the `.clap`.

## Atoms (human)

```text
[ATOM] type=decision | detail=watch = rebuild+install loop; preview = Slint interpreter
[ATOM] type=constraint | detail=cargo-aura stays dep-free
[ATOM] type=decision | detail=mesh wraps agal; agal_optional
```
