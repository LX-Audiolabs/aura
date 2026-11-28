---
name: aura-ui
description: AURA UI stack — Slint + aura-baseview / aura-editor / aura-build, @aura widgets, renderer backend features, and cargo-aura preview. Use for .slint files, widget work, editor interop, or UI preview in AURA.
---

# AURA UI (Slint + baseview)

AURA ships **Slint only**, windowed via **baseview**. For the Slint language
itself (syntax, layout, callbacks), load the global `slint` skill. This skill
covers AURA's UI-specific layer.

## Stack

```text
aura-editor    → Host Editor adapter (aura-core::Editor) — thin
aura-baseview  → window / platform / renderer only
upstream       → baseview, slint, femtovg / skia / wgpu
```

`aura-build` compiles `.slint` + provides `@aura` widgets. Reference:
`crates/aura-baseview/README.md`, `crates/aura-editor/`, `crates/aura-build/`.

## Renderer backends — exactly one

| Feature | Renderer |
|---------|----------|
| `backend-femtovg-gl` (**default**) | FemtoVG + OpenGL |
| `backend-femtovg-wgpu` | FemtoVG + wgpu |
| `backend-skia` | Skia |
| `backend-software` | software + wgpu blit |

Aliases: `backend-femtovg` → gl, `backend-wgpu` → software,
`backend-wgpu-vulkan` → software. Enable exactly one; `--all-features` /
semver-checks break on the `compile_error!` guard.

## Author rules

- UI reads **snapshots** only — never write directly into DSP state. Push
  parameter edits through `Params` (control thread), not into `process()`.
- Keep `process()` allocation- and lock-free; the editor may allocate.
- HiDPI/scale, keyboard, clipboard are handled by baseview; don't reimplement.
- Parameter gestures (`begin`/`end`) and automation must go through the host
  param path, not ad-hoc setters.

## Preview (no DAW)

```bash
cargo aura preview                 # hot-reload UI
cargo aura preview --component 1    # pick component N (multi-UI files)
cargo aura preview --no-watch       # one-shot
# or: cargo run -p aura-preview -- path/to/ui/main.slint
```

## Common fixes

- `.slint` compile errors → check `aura-build` version + `@aura` import path.
- Missing renderer → pick one backend feature (see table).
- Host GUI greyed out → only plugins with `PluginLogic::editor` expose a GUI.
