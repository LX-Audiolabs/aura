# aura-gui

Visual **project console** for AURA. Thin Slint shell over the same actions as:

```text
cargo aura new | init | add | build | install | doctor
```

CLI remains the source of truth — every button builds the same flag list.

## Run

```bash
# from AURA workspace
cargo run -p aura-gui

# or via cargo-aura
cargo aura gui
```

Requires `cargo-aura` on `PATH` **or** `AURA_PATH` / running inside the AURA repo (falls back to `cargo run -p cargo-aura`).

**Desktop Slint** pulls a larger dep graph than plugin crates. If `icu_*` / `rust-lld` crashes on Windows, repair the MSVC toolchain or temporarily drop the local `.cargo/config.toml` `rust-lld` workaround and use a fixed `link.exe`.

## Layout

| Control | CLI |
|---------|-----|
| New | `cargo aura new <name> [--vst3] [--lv2] --kind …` |
| Init | `cargo aura init <dir> …` |
| Add plugin | `cargo aura add <name> …` |
| Build / Install | `cargo aura build\|install --clap\|--vst3\|--lv2 [--release]` |
| Doctor | `cargo aura doctor` |

Long work runs on a worker thread; log streams into the window.
