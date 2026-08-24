<!-- AGAL:AUTO-START -->
# aura-hot

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura-hot` |
| description | CLAP hot-reload proxy — host maps this .clap; DSP lives in a sibling .impl that watch can replace |
| frameworks | clap |
| generated | `2026-08-24T16:17:07Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=crate id=crates/aura-hot
[ATOM] type=fact | detail=frameworks=clap
[ATOM] type=fact | detail=roles=entry+manifest
```

## structure
- public_api symbols: 4 (see json)
- roles: entry, manifest

## api surface
- `fn impl_path_beside(clap_path: &Path) -> PathBuf` · `src/lib.rs`
- `fn impl_suffix() -> &static str` · `src/lib.rs`
- `static clap_entry: clap_plugin_entry` · `src/lib.rs`
- `impl Send for Inner` · `src/lib.rs`

## findings
- [info] **crate_no_dependents**: aura-hot has no inbound workspace edges — unused or only path-included? · `crates/aura-hot` · fix: wire `aura-hot` as a path dep from a plugin/crate, or remove from workspace

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `crates/aura-hot` in json / `agal --plugin aura-hot .`

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
