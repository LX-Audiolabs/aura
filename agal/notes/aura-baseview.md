<!-- AGAL:AUTO-START -->
# aura-baseview

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura-baseview` |
| description | AURA Slint + baseview window stack (FemtoVG / Skia / software) — no plugin host API |
| frameworks | aura-baseview, baseview, raw-window-handle, slint |
| generated | `2026-08-27T17:47:49Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=crate id=crates/aura-baseview
[ATOM] type=fact | detail=frameworks=aura-baseview+baseview+raw-window-handle+slint
[ATOM] type=fact | detail=roles=entry+manifest+slint+source
[ATOM] type=fact | detail=used_by=aura-baseview/examples/open_parented via depends_on
[ATOM] type=fact | detail=used_by=aura-baseview/examples/render_femtovg via depends_on
[ATOM] type=fact | detail=used_by=aura-editor via depends_on
```

## dependents (inbound)
- `aura-baseview/examples/open_parented` --depends_on--> `aura-baseview`
- `aura-baseview/examples/render_femtovg` --depends_on--> `aura-baseview`
- `aura-editor` --depends_on--> `aura-baseview`

## structure
- public_api symbols: 49 (see json)
- roles: entry, manifest, slint, source

## api surface
- `struct BaseviewSlintWindowAdapter { … }` · `src/baseview_slint_window_adapter.rs`
- `struct GlInitError { message: String }` · `src/baseview_slint_window_adapter.rs`
- `struct BlitPipeline { … }` · `src/blit.rs`
- `struct SlintGlContext { gl_context: GlContext }` · `src/open_gl_interface.rs`
- … +45 more public symbols

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `crates/aura-baseview` in json / `agal --plugin aura-baseview .`

<!-- AGAL:AUTO-END -->

<!-- AGAL:HUMAN — edit below this line; preserved on regenerate -->

## Intent

Slint window stack on baseview (parented host view + GL). `aura-editor` is the
`Editor` adapter; this crate owns HWND/NSView/X11 + renderer backends.

## Open

- Enable **exactly one** renderer feature (`backend-femtovg` default, or
  skia / wgpu). Zero or multiple → `compile_error`.

## Decisions

- `SlintGlContext::ensure_current` must not return `Err` on a dead WGL DC.
  Bitwig can destroy the parent HWND before `gui_destroy`; Slint `Drop` then
  `expect`s `free_graphics_resources` and aborts in `wnd_proc` (`0xC000041D`).
  Lesson also recorded on `aura-editor` (host-facing).

## Atoms (human)

```text
[ATOM] type=constraint | detail=aura-baseview: enable exactly one of backend-femtovg / backend-skia / backend-wgpu
[ATOM] type=lesson | detail=ensure_current must not return Err on dead WGL DC (Bitwig parent HWND dies before gui_destroy)
```
