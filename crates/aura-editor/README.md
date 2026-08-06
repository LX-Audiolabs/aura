# aura-editor

Host **Editor** side of AURA UI (CLAP/DAW open-close, params bridge).

| Crate | Role |
|-------|------|
| **[`aura-baseview`](../aura-baseview)** | Window stack (Slint + baseview, renderer features) |
| **`aura-editor`** | This crate — re-exports baseview **now**; `aura_core::Editor` adapter **next** |

```toml
# Plugins that only need a window can depend on aura-baseview directly.
aura-baseview = { workspace = true, features = ["backend-femtovg"] }

# Prefer aura-editor once the host adapter exists (re-exports baseview today).
aura-editor = { workspace = true, features = ["backend-femtovg"] }
```

```rust
// Today (re-export):
use aura_editor::slint_window::SlintWindow;
// Same as:
use aura_baseview::slint_window::SlintWindow;
```

## License

**MIT** (same as `aura-baseview` for the re-export surface).
