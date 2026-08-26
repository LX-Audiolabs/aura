<!-- AGAL:AUTO-START -->
# aura-baseview

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura-baseview` |
| description | AURA Slint + baseview window stack (FemtoVG / Skia / software) — no plugin host API |
| frameworks | aura-baseview, baseview, raw-window-handle, slint |
| generated | `2026-08-26T19:58:10Z` |

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
