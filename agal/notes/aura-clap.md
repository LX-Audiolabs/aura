<!-- AGAL:AUTO-START -->
# aura-clap

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura-clap` |
| description | CLAP format wrapper for AURA — free-audio/clap via clap-sys (minimal v1) |
| frameworks | aura, clap |
| generated | `2026-08-18T12:06:01Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=crate id=crates/aura-clap
[ATOM] type=fact | detail=frameworks=aura+clap
[ATOM] type=fact | detail=roles=entry+manifest+state
[ATOM] type=fact | detail=depends_on=aura-core
[ATOM] type=fact | detail=depends_on=aura-params
[ATOM] type=fact | detail=used_by=aura via depends_on
```

## deps (workspace)
- `aura-core`
- `aura-params`

## dependents (inbound)
- `aura` --depends_on--> `aura-clap`

## structure
- public_api symbols: 6 (see json)
- roles: entry, manifest, state

## api surface
- `fn entry_deinit()` · `src/lib.rs`
- `fn entry_init(_plugin_path: *const c_char) -> bool` · `src/lib.rs`
- `fn get_factory<L>(factory_id: *const c_char) -> *const c_void` · `src/lib.rs`
- `impl EditorBridge for ClapBridge` · `src/lib.rs`
- … +2 more public symbols

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `crates/aura-clap` in json / `agal --plugin aura-clap .`

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
