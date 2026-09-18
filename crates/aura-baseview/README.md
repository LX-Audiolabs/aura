# aura-baseview

**AURA** windowing layer: **[Slint](https://slint.dev) + [baseview](https://github.com/RustAudio/baseview)**.

Lineage: `lx-slint-baseview` (LX Audiolabs). Built because stock / truce Slint paths did not cover DAW embed needs (host scale, parented windows, clipboard, multi-renderer).

**Not** the same crate as BillyDM / crates.io `slint-baseview` (different baseview generation, multi-backend, DAW focus).

## Layering

```text
aura-editor     →  Host Editor adapter (aura-core::Editor) — thin, depends on this crate
aura-baseview   →  THIS: window / platform / renderer only
upstream        →  baseview, slint, femtovg/skia/wgpu
```

Upstream upgrades (baseview / Slint / renderer) are version bumps **here**, not mixed into the host editor.

## Features

Exactly one canonical renderer:

| Feature | Renderer |
|---------|----------|
| `backend-femtovg-gl` (**default**) | FemtoVG + OpenGL |
| `backend-femtovg-wgpu` | FemtoVG + wgpu (GPU, no software blit) |
| `backend-skia` | Skia |
| `backend-software` | Software renderer + wgpu blit |

Transition aliases (same as enabling the canonical feature):

| Alias | Resolves to |
|-------|-------------|
| `backend-femtovg` | `backend-femtovg-gl` |
| `backend-wgpu` | `backend-software` |
| `backend-wgpu-vulkan` | `backend-software` + Vulkan-only wgpu backends |

```toml
lx-aura-baseview = { version = "0.14", features = ["backend-femtovg-wgpu"] }
# or keep the old name during cutover:
lx-aura-baseview = { version = "0.14", features = ["backend-femtovg"] }
```

```rust
use aura_baseview::platform;
use aura_baseview::slint_window::SlintWindow;
```

## License

**MIT** — see [`LICENSE-MIT`](./LICENSE-MIT).  

**crates.io:** prepared with Tier B (`docs/crates-io-prep.md`); `publish = false`
until the explicit first-release decision. Distinct from BillyDM
`slint-baseview`.

## Examples

```bash
cargo run -p aura-example-render-femtovg
cargo run -p aura-example-open-parented
```
