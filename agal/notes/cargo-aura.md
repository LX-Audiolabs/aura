<!-- AGAL:AUTO-START -->
# cargo-aura

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `member` |
| path | `tools/cargo-aura` |
| description | Build tool for AURA audio plugins — cargo aura new|build|install|watch|mesh|doctor |
| generated | `2026-08-29T13:54:23Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=member id=tools/cargo-aura
[ATOM] type=fact | detail=roles=entry+manifest+source
```

## structure
- roles: entry, manifest, source

## api surface
- `struct ScaffoldSpec { … }` · `src/scaffold.rs`
- `enum Kind` · `src/scaffold.rs`
- `fn append_plugin_table(text: &str, block: &str) -> String` · `src/scaffold.rs`
- `fn aura_toml_has_bundle(text: &str, bundle_id: &str) -> bool` · `src/scaffold.rs`
- … +7 more public symbols

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `tools/cargo-aura` in json / `agal --plugin cargo-aura .`

<!-- AGAL:AUTO-END -->

<!-- AGAL:HUMAN — edit below this line; preserved on regenerate -->

## Intent

Author CLI: scaffold, build, install, watch, preview, doctor. Dev-host loop is
`cargo aura run` → `aura-host`. Orientation mesh is
`cargo aura mesh` → `agal` (optional). `tools/aura-gui` deleted (2026-08-28).

## Open

- [x] In-host swap without unload — `cargo aura watch --hot` (`aura-hot` proxy + `.impl`); re-add instance
- [x] `cargo aura run` — launches `aura-host` (`aura-gui` removed)

## Decisions

- `watch` polls mtimes with std only (no `notify` dep). `preview` already uses `notify` for Slint.
- Default watch format is `--clap`. `--no-install` builds without copying.
- `--hot` installs `aura-hot` as `Name.clap` and the real plugin as `Name.impl.*`. Host never maps the impl, so watch can overwrite it. Re-add instance to run new DSP.
- `mesh` is not a doctor gate. Builds work without agal.
- Install copy retries when the host still maps the `.clap`.

## Atoms (human)

```text
[ATOM] type=decision | detail=watch = rebuild+install loop; preview = Slint interpreter
[ATOM] type=decision | detail=run = launch aura-host; aura-gui tree removed 2026-08-28
[ATOM] type=constraint | detail=cargo-aura stays dep-free
[ATOM] type=decision | detail=mesh wraps agal; agal_optional
```
