# aura-clap

CLAP format wrapper for AURA.

## Spec source of truth

**We follow [free-audio/clap](https://github.com/free-audio/clap)** — the Clever Audio Plugin API (headers under `include/clap/`).

| Layer | What |
|-------|------|
| **Spec / ABI** | free-audio/clap (`CLAP_VERSION` on their `main` / releases) |
| **Rust bindings** | [`clap-sys`](https://crates.io/crates/clap-sys) — C layout + constants from those headers |
| **Our code** | `export!`, factory, process, audio-ports (+ config, sidechain), note-ports, params (sample-accurate + mono mod), state, GUI, remote-controls, latency, tail, render, **preset-load** (+ discovery when factory presets exist) |

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

Entry, factory, audio-ports (mono/stereo + optional sidechain), audio-ports-config, note-ports, params, process, state, GUI, remote-controls, latency, **tail**, **render**, **preset-load**, **tuning** (`clap.tuning/2`, MTS-ESP host support).

**Layouts:** `PluginLogic::bus_layouts()` — default stereo. Override with `BusLayout::mono()`, `stereo_and_mono()`, or `.with_sidechain(…)`. Host switches via `clap.audio-ports-config` when more than one layout is declared.

**Sample-accurate automation:** host `PARAM_VALUE` / `PARAM_MOD` with `time > 0` split the block for params with `ParamFlags::CHUNKED` (default). Opt out with `#[param(chunk = false)]`. See `aura_core::chunked_process`.

**Modulation:** `#[param(flags = "modulatable")]` → `CLAP_PARAM_IS_MODULATABLE`. Host `PARAM_MOD` is a non-destructive offset; DSP reads `clamp(base + mod)` via smoothers; host UI/`get` stays on base. `note_id < 0` is mono (params). `note_id ≥ 0` is poly — delivered on `ProcessContext.notes` as `NoteEventKind::ParamMod` (does not overwrite mono `set_mod`). Advertise `#[param(flags = "modulatable | modulatable_per_note")]` so hosts send per-note mods.

**Note events / expressions:** prefer `MidiDialect::Clap` so the note port lists `CLAP_NOTE_DIALECT_CLAP`. Wrapper fills `ProcessContext.notes` with on/off/choke (`note_id`, key, velocity 0..1), `CLAP_EVENT_NOTE_EXPRESSION` (volume/pan/tuning/…), and still mirrors on/off into 7-bit `midi`.

**Note out:** push to `ProcessContext.notes_out`. CLAP emits `NOTE_ON`/`OFF`/`CHOKE`/`END` and expressions. `midi_out` is always raw `CLAP_EVENT_MIDI` (Bitwig Note FX forwards that dialect). `NOTE_END` (voice silent) needs no output note port — Bitwig can drop poly mods. Generated notes (arp / seq) need `PluginInfo.emits_midi` so a note output port exists. VST3/LV2 map On/Off/Choke to 7-bit MIDI.

**Voice info:** set `PluginInfo.voice_count` / `voice_capacity` (`> 0`) to advertise `clap.voice-info` with overlapping notes. Bitwig Voice Stack stays mono-mapped without this.

**Latency:** override `PluginLogic::latency(&state) -> u32` (samples). Via `clap.latency`; mid-run changes request host restart for PDC.

**Tail:** override `PluginLogic::tail_length(&state) -> u32`. Via `clap.tail` (+ VST3 `getTailSamples`).

**Render:** `clap.render` sets `ProcessContext.process_mode` (`Realtime` / `Offline`).

**Preset load:** `clap.preset-load/2` (compat `clap.preset-load.draft/2`). FILE location reads a v1 state blob (`PluginLogic::load_preset_from_file`). PLUGIN location looks up `PluginLogic::factory_presets` by `load_key`. Non-empty factory list also exposes `preset-discovery-factory/2` (PLUGIN / factory content).

**Tuning / MTS-ESP:** set `PluginInfo.supports_tuning = true` to advertise `clap.tuning/2`. In `process()`, query `context.tuning.relative_offset(port, channel, key, sample_offset)` for the detune in semitones and `context.tuning.should_play(...)` to honor host-driven note filtering. Host tuning changes split the block sample-accurately; `PluginLogic::tuning_changed` is called when the host reports that the tuning pool changed.

**MIDI 2:** set `PluginInfo.midi_input_dialect = MidiDialect::Midi2` to prefer `CLAP_NOTE_DIALECT_MIDI2` (CLAP + MIDI 1 still advertised). Incoming `CLAP_EVENT_MIDI2` lands on `ProcessContext.ump` as native [`Ump`] packets (per-note pitch bend, SysEx8, Flex included). A 7-bit image is still mirrored to `midi` when `to_midi1` exists. Push generated packets to `ump_out` — CLAP emits `CLAP_EVENT_MIDI2`. Native notes / expressions stay on `MidiDialect::Clap` + `ProcessContext.notes`.

**Hot reload:** `cargo aura watch --hot` installs `aura-hot` as `Name.clap` and the real plugin as `Name.impl.dll` (or `.so` / `.dylib`). The host keeps the proxy mapped; watch overwrites the impl. Re-add the instance to run the new DSP.

**Remote-controls:** pages of ≤8 params from `ParamInfo.group`. Empty group = no device page. `"Section/Page"` splits on the first `/`. Hidden/readonly never take a hardware slot.

**Validator:** in-tree smokes (`smoke-gain`, `smoke-midi-fx`, `smoke-sidechain`, `smoke-synth`) — clap-validator **0 failed** (2026-08-11).

---

## Outstanding (CLAP) — do not forget

Ship-capable CLAP core is **done**. Remaining work is **product-driven** or optional host proof — not basis holes.

### Product-driven (implement only when a plugin needs it)

| Item | Gap / notes | Trigger |
|------|-------------|---------|
| ~~**`clap.preset-load`**~~ | landed | `factory_presets` + FILE blob load; vault-format files override `load_preset_from_file` |
| ~~**Poly param modulation**~~ | landed — `note_id ≥ 0` → `ProcessContext.notes` (`ParamMod`); smoke-synth Gain is `modulatable_per_note` | plugin owns voice table |
| ~~**Note expression**~~ | landed — `CLAP_EVENT_NOTE_EXPRESSION` → `NoteEventKind::Expression`; prefer `MidiDialect::Clap` | Bitwig expression / MPE host proof |
| ~~**Native MIDI 2 process**~~ | landed v0.7.2 — `ProcessContext.ump` / `ump_out`; `NoteVoiceTable` + `NOTE_END` | plugin owns envelopes; call `mark_silent` |
| ~~**Multi-out (one aux)**~~ | landed — `BusLayout::with_aux` + `smoke-aux`; REAPER: 2 in / 4 out (2 main + 2 aux) OK 2026-08-29 | product may still want >1 aux / >1 SC |
| **>1 sidechain / N aux** | still one SC in + one aux out max | more buses than the single optional pair |
| **Rich state hooks** (host blob > flat params) | G5 | Presets that need non-param bytes in host state |
| **SysEx typed decode** | raw `SysEx8` / Flex already on `ump` | hardware bridge that needs a typed API |

### Host proof (framework landed; Bitwig re-check 2026-08-28)

Install: `cargo aura install --clap --release -plug smoke-synth smoke-midi-fx smoke-gain smoke-sidechain`

- [x] **Poly** — `AURA Smoke Synth` on an instrument track. Play a chord (4–8 keys). All notes sound. Steal after 8.
- [x] **Expressions** — Bitwig note inspector: Gain (volume), Timbre (sine→saw), Pressure. Per-note, not global.
- [x] **Poly-mod / Voice Stack** — Bitwig Voice Stack on Gain (per-note). Stacked copies of the same key must differ in level. After release, mods drop (`NOTE_END`).
- [x] **Mono-mod** — same modulator without per-note: whole instance Gain moves, UI value stays.
- [x] **`notes_out`** — Note FX chain: `AURA Smoke MIDI FX` (Transpose +12) → `AURA Smoke Synth`. Played C sounds as C one octave up.
- [~] **Layouts** — switch stereo/mono on the instrument (smoke-synth declares both). *N/A in Bitwig — often no CLAP layout picker; `audio-ports-config` still advertised.*
- [x] **Offline** — bounce the clip; no hang. (`clap.render` → `ProcessMode::Offline`)
- [x] **Sidechain** — `AURA Smoke Sidechain` (Audio FX + mono SC). Bitwig: SC route audible via Amount mix (2026-08-28).
- [ ] Multi-page `remote-controls` / non-zero `latency` / mid-block automation — use `smoke-gain` + a later product plug; not blocking the note path.

**aura-host GUI note:** only plugins with `PluginLogic::editor` enable “Open plugin GUI” (`smoke-gain`). `smoke-synth` is headless — button stays disabled. AURA rejects CLAP floating; host embeds on Windows.

### Bitwig session (copy)

1. Rescan CLAP. Insert **AURA Smoke Synth**. Play a triad — three distinct pitches.
2. Open the note inspector. Move Gain / Timbre on one note only.
3. Inspector → Voice Stacking (e.g. 4). Add **Voice Stack** modulator on Gain, spread amount. Stacked copies of one key should be different levels. Re-add the plugin after this rebuild (flags changed).
4. Insert **AURA Smoke MIDI FX** in the **Note FX** chain (not audio FX) *before* the synth. Transpose 12. Synth plays an octave up. Re-add MIDI FX after this rebuild.
5. Bounce the clip offline.

### Spec / bindings (not a ship blocker)

- `clap-sys` may lag free-audio 1.2.x revision — bump only when a **new** extension is needed (see version snapshot above).
- Do **not** claim “full CLAP zoo” (thread-pool, surround/ambisonic ports, context-menu, track-info, …) unless product requires it.

### Explicit non-goals (AURA)

- AU / AAX / VST2  
- Multi-UI toolkits  

Outstanding items above are product-driven — implement when a plugin needs them, not as framework completeness.
