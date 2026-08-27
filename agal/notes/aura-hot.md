<!-- AGAL:AUTO-START -->
# aura-hot

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura-hot` |
| description | CLAP hot-reload proxy — host maps this .clap; DSP lives in a sibling .impl that watch can replace |
| frameworks | clap |
| generated | `2026-08-27T16:56:27Z` |

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

CLAP hot-reload **proxy**. Host maps `Name.clap` (this crate). Real plugin is
sibling `Name.impl.*` that `cargo aura watch --hot` overwrites. Re-add the
instance to run new DSP — existing instances keep their generation.

## Open

- `crate_no_dependents` is expected — cargo-aura installs the binary, nothing
  in-tree depends on the library.

## Decisions

- Each load copies the impl to a unique temp file so Windows does not lock it.
  Inner vtables stay mapped (dev leak is fine).

## Atoms (human)

```text
[ATOM] type=decision | detail=aura-hot is the mapped Name.clap proxy; watch overwrites Name.impl.*; re-add instance to swap DSP
```
