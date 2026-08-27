<!-- AGAL:AUTO-START -->
# aura-editor

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura-editor` |
| description | AURA host Editor adapter (Slint on aura-baseview) for CLAP/VST3/LV2 GUI |
| frameworks | aura, aura-baseview, aura-editor, baseview, raw-window-handle, slint |
| generated | `2026-08-27T06:03:56Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=crate id=crates/aura-editor
[ATOM] type=fact | detail=frameworks=aura+aura-baseview+aura-editor+baseview+raw-window-handle+slint
[ATOM] type=fact | detail=roles=entry+manifest+source
[ATOM] type=fact | detail=depends_on=aura-baseview
[ATOM] type=fact | detail=depends_on=aura-core
[ATOM] type=fact | detail=depends_on=aura-params
[ATOM] type=fact | detail=used_by=examples/smoke-gain via depends_on
```

## deps (workspace)
- `aura-baseview`
- `aura-core`
- `aura-params`

## dependents (inbound)
- `examples/smoke-gain` --depends_on--> `aura-editor`

## structure
- public_api symbols: 25 (see json)
- roles: entry, manifest, source

## api surface
- `trait PluginContextReadF32` · `src/typed.rs`
- `struct AuraSlintEditor<C> { … }` · `src/lib.rs`
- `struct LxPluginContext<P> { params: Arc<P>, host: PluginContext }` · `src/typed.rs`
- `struct LxSlintEditor<P, C> { … }` · `src/typed.rs`
- … +23 more public symbols

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `crates/aura-editor` in json / `agal --plugin aura-editor .`

<!-- AGAL:AUTO-END -->

<!-- AGAL:HUMAN — edit below this line; preserved on regenerate -->

## Intent

Host-facing `Editor` adapter: Slint component on `aura-baseview`, exposed as
`aura_core::Editor` so CLAP/VST3/LV2 can parent it. Window/GL stack stays in
`aura-baseview`; this crate is the host glue (`AuraSlintEditor`).

## Open

- None in this crate. Embed socket position / resize is `aura-host`.

## Decisions

- Always `AuraSlintEditor` — never raw `slint::Window` in a plugin.
- `on_init` builds the component and wires param **writes**. `on_idle` is
  one-way params → UI (epsilon guard). Gestures stay off the audio thread.
- `@aura` basics (Knob, Slider, Toggle, Dropdown, Meter, XYPad) live in
  `aura-build/ui/`. PeakMeter / FFT / Spectrum stay the **product** design
  system (`lx-ui-slint`).
- AURA wrappers reject floating GUI (`is_floating = true`). Embed only.
- Bitwig can destroy the parent HWND *before* `gui_destroy`. Child `WM_DESTROY`
  drops Slint → `free_graphics_resources.expect`. `SlintGlContext::ensure_current`
  must **not** return `Err`; `catch_unwind` around `Editor::close` cannot catch
  a `wnd_proc` abort (`0xC000041D`).

## Atoms (human)

```text
[ATOM] type=constraint | detail=PeakMeter/FFT/Spectrum widgets stay product design system; @aura basics incl XYPad only
[ATOM] type=lesson | detail=Bitwig UI-close sandbox abort (0xC000041D): parent HWND dies before clap gui_destroy → child WM_DESTROY → Slint Drop → free_graphics_resources.expect. ensure_current must not return Err; catch_unwind around Editor::close cannot catch wnd_proc abort
```
