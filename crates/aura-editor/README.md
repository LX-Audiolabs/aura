# aura-editor

Host **Editor** adapter for [AURA](https://github.com/LX-Audiolabs/aura):
bridges Slint UI on [`aura-baseview`](https://crates.io/crates/aura-baseview)
to `aura_core::Editor` for CLAP / VST3 / LV2.

```toml
aura-editor = { version = "0.11", features = ["backend-femtovg"] }
aura-baseview = { version = "0.11", features = ["backend-femtovg"] }
```

License: **MIT** (see `LICENSE-MIT`). Core/params remain GPL when linked into a
plugin — see repository `docs/licensing-compliance.md`.
