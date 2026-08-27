<!-- AGAL:AUTO-START -->
# aura-build

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura-build` |
| description | AURA build helper: @aura Slint widgets + bundled fonts for slint-build 1.17.1 |
| frameworks | slint |
| generated | `2026-08-27T16:56:27Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=crate id=crates/aura-build
[ATOM] type=fact | detail=frameworks=slint
[ATOM] type=fact | detail=roles=entry+manifest+slint
[ATOM] type=fact | detail=used_by=aura-host via build_depends_on
[ATOM] type=fact | detail=used_by=examples/smoke-gain via build_depends_on
[ATOM] type=fact | detail=used_by=tools/aura-gui via build_depends_on
```

## dependents (inbound)
- `aura-host` --build_depends_on--> `aura-build`
- `examples/smoke-gain` --build_depends_on--> `aura-build`
- `tools/aura-gui` --build_depends_on--> `aura-build`
- `tools/aura-preview` --depends_on--> `aura-build`

## structure
- public_api symbols: 7 (see json)
- roles: entry, manifest, slint

## api surface
- `struct AssetPaths { … }` · `src/lib.rs`
- `enum CompileError` · `src/lib.rs`
- `fn compile(slint_entry: impl AsRef<Path>) -> Result<(),CompileError>` · `src/lib.rs`
- `fn materialize_assets(dir: &Path) -> Result<AssetPaths,CompileError>` · `src/lib.rs`
- … +3 more public symbols

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `crates/aura-build` in json / `agal --plugin aura-build .`

<!-- AGAL:AUTO-END -->

<!-- AGAL:HUMAN — edit below this line; preserved on regenerate -->

## Intent

Plugin build helpers: `materialize_assets!` copies `.slint` → `OUT_DIR` and
sets `SLINT_LIBRARY_PATHS` so `@aura` resolves. Portable widgets live in
`aura-build/ui/` (Knob, Slider, Toggle, Dropdown, Meter, XYPad, theme).

## Open

- None. Product PeakMeter/FFT/Spectrum stay in `lx-ui-slint`.

## Decisions

- Plugin `cdylib` uses `aura_build::materialize_assets!` + `slint::include_modules!()`.
  UI library crates may call `slint_build::compile` directly.

## Atoms (human)

```text
[ATOM] type=decision | detail=@aura widgets ship in aura-build/ui/; PeakMeter/FFT/Spectrum stay product (lx-ui-slint)
```
