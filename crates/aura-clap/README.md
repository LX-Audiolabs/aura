# aura-clap

CLAP format wrapper for AURA.

## Spec source of truth

**We follow [free-audio/clap](https://github.com/free-audio/clap)** — the Clever Audio Plugin API (headers under `include/clap/`).

| Layer | What |
|-------|------|
| **Spec / ABI** | free-audio/clap (`CLAP_VERSION` on their `main` / releases) |
| **Rust bindings** | [`clap-sys`](https://crates.io/crates/clap-sys) — C layout + constants from those headers |
| **Our code** | `export!`, factory, process, audio-ports, params (growing) |

Rules:

1. **New extensions and behaviour** come from free-audio docs/headers, not from truce or folklore.
2. **`clap-sys` may lag** the latest free-audio **revision** (1.2.x). Major/minor 1.2 stay ABI-compatible (`clap_version_is_compatible`: major ≥ 1). We still design against free-audio; bump `clap-sys` when bindings catch up or switch to git/bindgen if we need a newer header.
3. **Do not invent** extension IDs or struct layouts — copy free-audio names.

### Version snapshot (check when bumping)

| | Version |
|--|---------|
| free-audio/clap (upstream) | **1.2.10** (as of check against `main` `version.h`) |
| clap-sys on crates.io `0.5` | reports **1.2.2** in `CLAP_VERSION_*` |
| clap-sys git master | has been at **1.2.3** |

Hosts accept 1.2.x plugins as CLAP 1.x. Track free-audio for **new** extensions; do not block development on revision parity alone.

## Usage

```toml
aura = { path = "...", features = ["clap"] }
```

```rust
#[cfg(feature = "clap")]
aura::export!(MyPlugin);
```

## Status

Minimal: entry, factory, stereo audio-ports, params, process.  
Later: state, GUI, note-ports, latency, … per free-audio ext headers + clap-validator.
