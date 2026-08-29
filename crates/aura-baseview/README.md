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

| Always | Choose |
|--------|--------|
| Slint UI | `backend-femtovg` (default, OpenGL) |
| baseview host window | `backend-skia` |
| | `backend-wgpu` (software + wgpu blit) |

```toml
aura-baseview = { path = "...", features = ["backend-femtovg"] }
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
