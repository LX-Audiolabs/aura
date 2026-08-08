# aura-clap

CLAP format wrapper for AURA.

## Spec source of truth

**We follow [free-audio/clap](https://github.com/free-audio/clap)** — the Clever Audio Plugin API (headers under `include/clap/`).

| Layer | What |
|-------|------|
| **Spec / ABI** | free-audio/clap (`CLAP_VERSION` on their `main` / releases) |
| **Rust bindings** | [`clap-sys`](https://crates.io/crates/clap-sys) — C layout + constants from those headers |
| **Our code** | `export!`, factory, process, audio-ports (+ config), params, state, GUI, remote-controls |

Rules:

1. **New extensions and behaviour** come from free-audio docs/headers (product needs decide *which* extensions we ship).
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

Entry, factory, audio-ports (mono/stereo layouts), audio-ports-config (when multi-layout), params, process, state, GUI, remote-controls.

**Layouts:** `PluginLogic::bus_layouts()` — default stereo. Override with `BusLayout::mono()` or `BusLayout::stereo_and_mono()`. Host switches via `clap.audio-ports-config` when more than one layout is declared.

**Remote-controls:** pages of ≤8 params from `ParamInfo.group`. Empty group = no device page. `"Section/Page"` splits on the first `/`. Hidden/readonly never take a hardware slot.

Later: note-ports, latency, multi-bus / sidechain, … per free-audio + product need.
