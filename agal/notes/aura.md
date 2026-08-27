<!-- AGAL:AUTO-START -->
# aura

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura` |
| description | AURA — Audio Unified Rust Architecture (umbrella crate) |
| frameworks | aura, clap, lv2, vst3 |
| generated | `2026-08-27T16:56:27Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=crate id=crates/aura
[ATOM] type=fact | detail=frameworks=aura+clap+lv2+vst3
[ATOM] type=fact | detail=roles=entry+manifest
[ATOM] type=fact | detail=depends_on=aura-clap
[ATOM] type=fact | detail=depends_on=aura-core
[ATOM] type=fact | detail=depends_on=aura-derive
[ATOM] type=fact | detail=used_by=examples/smoke-gain via depends_on
[ATOM] type=fact | detail=used_by=examples/smoke-midi-fx via depends_on
[ATOM] type=fact | detail=used_by=examples/smoke-sidechain via depends_on
```

## deps (workspace)
- `aura-clap`
- `aura-core`
- `aura-derive`
- `aura-dsp`
- `aura-lv2`
- `aura-midi`
- `aura-params`
- `aura-vst3`

## dependents (inbound)
- `examples/smoke-gain` --depends_on--> `aura`
- `examples/smoke-midi-fx` --depends_on--> `aura`
- `examples/smoke-sidechain` --depends_on--> `aura`
- `examples/smoke-synth` --depends_on--> `aura`
- `aura-lv2` --dev_depends_on--> `aura`

## structure
- params: CollidingParams (1 fields)
- params: GainParams (4 fields)
- params: SubParams (1 fields)
- params: TapParams (1 fields)
- params: TestParams (5 fields)
- public_api symbols: 1 (see json)
- roles: entry, manifest

## api surface
- `pub mod prelude` · `src/lib.rs`

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `crates/aura` in json / `agal --plugin aura .`

<!-- AGAL:AUTO-END -->

<!-- AGAL:HUMAN — edit below this line; preserved on regenerate -->

## Intent

Umbrella crate for plugin authors. `use aura::prelude::*` + format features
(`clap` / `vst3` / `lv2`). Do not depend on every `aura-*` piece by hand.

## Open

- None. Format leftovers live in `aura-clap`.

## Decisions

- Re-exports `aura-core` / `dsp` / `midi` / `params` and, behind features,
  the thin format wrappers. `aura-test` is a **dev-dep**, not a feature.

## Atoms (human)

```text
[ATOM] type=decision | detail=aura is the author umbrella — prelude + format features; aura-test is a dev-dep not a feature
```
