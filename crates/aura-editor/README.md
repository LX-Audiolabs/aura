# aura-editor

Host **Editor** adapter for [AURA](https://github.com/LX-Audiolabs/aura):
bridges Slint UI on [`aura-baseview`](https://crates.io/crates/aura-baseview)
to `aura_core::Editor` for CLAP / VST3 / LV2.

```toml
lx-aura-editor = { version = "0.14", default-features = false, features = ["backend-femtovg-wgpu"] }
# aliases still work: backend-femtovg → FemtoVG+GL, backend-wgpu → software blit
```

Renderer features are forwarded from `lx-aura-baseview` — see that crate's README for the matrix (`backend-femtovg-gl` / `backend-femtovg-wgpu` / `backend-skia` / `backend-software`).

License: **MIT** (see `LICENSE-MIT`). Core/params remain GPL when linked into a
plugin — see repository `docs/licensing-compliance.md`.
