# aura-clap

CLAP format wrapper for AURA.

## Spec source of truth

**We follow [free-audio/clap](https://github.com/free-audio/clap)** — the Clever Audio Plugin API (headers under `include/clap/`).

| Layer | What |
|-------|------|
| **Spec / ABI** | free-audio/clap (`CLAP_VERSION` on their `main` / releases) |
| **Rust bindings** | [`clap-sys`](https://crates.io/crates/clap-sys) — C layout + constants from those headers |
| **Our code** | `export!`, factory, process, audio-ports (+ config, sidechain), note-ports, params (sample-accurate + mono mod), state, GUI, remote-controls, latency, tail, render |

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

Entry, factory, audio-ports (mono/stereo + optional sidechain), audio-ports-config, note-ports, params, process, state, GUI, remote-controls, latency, **tail**, **render**.

**Layouts:** `PluginLogic::bus_layouts()` — default stereo. Override with `BusLayout::mono()`, `stereo_and_mono()`, or `.with_sidechain(…)`. Host switches via `clap.audio-ports-config` when more than one layout is declared.

**Sample-accurate automation:** host `PARAM_VALUE` / `PARAM_MOD` with `time > 0` split the block for params with `ParamFlags::CHUNKED` (default). Opt out with `#[param(chunk = false)]`. See `aura_core::chunked_process`.

**Modulation:** `#[param(flags = "modulatable")]` → `CLAP_PARAM_IS_MODULATABLE`. Host `PARAM_MOD` is a non-destructive offset; DSP reads `clamp(base + mod)` via smoothers; host UI/`get` stays on base. Mono first; per-note id is accepted only for mono (`note_id < 0`).

**Latency:** override `PluginLogic::latency(&state) -> u32` (samples). Via `clap.latency`; mid-run changes request host restart for PDC.

**Tail:** override `PluginLogic::tail_length(&state) -> u32`. Via `clap.tail` (+ VST3 `getTailSamples`).

**Render:** `clap.render` sets `ProcessContext.process_mode` (`Realtime` / `Offline`).

**Remote-controls:** pages of ≤8 params from `ParamInfo.group`. Empty group = no device page. `"Section/Page"` splits on the first `/`. Hidden/readonly never take a hardware slot.

**Validator:** in-tree smokes (`smoke-gain`, `smoke-midi-fx`, `smoke-sidechain`, `smoke-synth`) — clap-validator **0 failed** (2026-08-11).

---

## Outstanding (CLAP) — do not forget

Ship-capable CLAP core is **done**. Remaining work is **product-driven** or optional host proof — not basis holes.

### Product-driven (implement only when a plugin needs it)

| Item | Gap / notes | Trigger |
|------|-------------|---------|
| **`clap.preset-load`** (+ compat id if needed) | G14 rest | Factory presets in host browser / vault ship |
| **Poly param modulation** (`CLAP_PARAM_IS_MODULATABLE_PER_NOTE_ID`) | G18 poly — flag maps to CLAP; process drops `PARAM_MOD` with `note_id ≥ 0` until voice routing | Instrument with per-voice params |
| **Note expression** (CLAP note expression events) | Not wired to `ProcessContext` | MPE / Bitwig expression pilot |
| **Multi-out / >1 sidechain** | G12 extension — one optional sidechain only today | Aux buses beyond single SC |
| **Rich state hooks** (host blob > flat params) | G5 | Presets that need non-param bytes in host state |
| **SysEx** typed path | — | Rare / hardware bridge |

### Host proof (framework landed; DAW re-check recommended)

- [ ] Bitwig: multi-page `remote-controls`, non-zero `latency`, mid-block automation, modulator → `modulatable` param
- [ ] Mono / dual-layout host switch (`audio-ports-config`)
- [ ] Offline bounce sees `ProcessMode::Offline` (`clap.render`)

### Spec / bindings (not a ship blocker)

- `clap-sys` may lag free-audio 1.2.x revision — bump only when a **new** extension is needed (see version snapshot above).
- Do **not** claim “full CLAP zoo” (thread-pool, surround/ambisonic ports, context-menu, track-info, …) unless product requires it.

### Explicit non-goals (AURA)

- AU / AAX / VST2  
- Multi-UI toolkits  

Outstanding items above are product-driven — implement when a plugin needs them, not as framework completeness.
