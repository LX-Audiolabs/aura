<!-- AGAL:AUTO-START -->
# aura-params

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura-params` |
| description | AURA parameter system (ranges, smoothers, atomic params) |
| generated | `2026-08-27T17:47:49Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=crate id=crates/aura-params
[ATOM] type=fact | detail=roles=entry+manifest+source
[ATOM] type=fact | detail=used_by=aura via depends_on
[ATOM] type=fact | detail=used_by=aura-clap via depends_on
[ATOM] type=fact | detail=used_by=aura-core via depends_on
```

## dependents (inbound)
- `aura` --depends_on--> `aura-params`
- `aura-clap` --depends_on--> `aura-params`
- `aura-core` --depends_on--> `aura-params`
- `aura-dsp` --depends_on--> `aura-params`
- `aura-editor` --depends_on--> `aura-params`
- `aura-lv2` --depends_on--> `aura-params`
- `aura-test` --depends_on--> `aura-params`
- `aura-vst3` --depends_on--> `aura-params`

## structure
- public_api symbols: 41 (see json)
- roles: entry, manifest, source

## api surface
- `trait Params` · `src/lib.rs`
- `trait Sealed` · `src/lib.rs`
- `trait Float` · `src/sample.rs`
- `trait Sample` · `src/sample.rs`
- … +38 more public symbols

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `crates/aura-params` in json / `agal --plugin aura-params .`

<!-- AGAL:AUTO-END -->

<!-- AGAL:HUMAN — edit below this line; preserved on regenerate -->

## Intent

Automatable parameter surface + lock-free DSP↔UI taps. Hosts see `Params` via
`#[derive(Params)]` (`id = N` required). FFT/spectrum math stays product-side;
this crate only moves values and raw samples across the thread boundary.

## Open

- None. G5 rich (non-param) host state is a format leftover, not a params hole.

## Decisions

- Every `#[param]` field needs a unique `id = N` — wire-stable automation / state.
  Derive is required; no manual `impl Params` (`Sealed`).
- `AudioTap` (G15): lock-free SPSC sample ring. Audio thread `push`, UI `drain`.
  Declare `#[skip]`. Overflow overwrites oldest samples — never blocks, never grows.
  Default capacity 4096. Not part of `dyn Params`.
- Smoothing: `Smoother` / `SmoothingStyle` here; DSP reads smoothed values.
  Do not write the raw param into the sample loop.
- State blob is the flat param list. `AudioTap` / `#[skip]` fields are not
  serialized as automatable params.

## Atoms (human)

```text
[ATOM] type=decision | detail=G15 AudioTap landed 2026-08-10 — lock-free SPSC sample ring in aura-params
[ATOM] type=constraint | detail=AudioTap is #[skip] + SPSC (audio push / UI drain); not part of dyn Params or the host state blob
```
