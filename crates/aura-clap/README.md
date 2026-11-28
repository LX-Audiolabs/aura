# aura-clap

CLAP format wrapper for AURA.

Implementation and verification snapshot: **0.15.0**, **2026-11-28**.
The public [CLAP support matrix](../../CLAP-SUPPORT.md) is the coverage index:
supported / partial / not implemented, opt-ins, tests, and separate host evidence.

## Spec source of truth

**We follow [free-audio/clap](https://github.com/free-audio/clap)** — the Clever Audio Plugin API (headers under `include/clap/`).

| Layer | What |
|-------|------|
| **Spec / ABI** | free-audio/clap (`CLAP_VERSION` on their `main` / releases) |
| **Rust bindings** | crates.io [`clap-sys`](https://crates.io/crates/clap-sys) 0.6.0 today. Maintainer [fork](https://github.com/lxndrbe/clap-sys) integration remains planned; no in-tree takeover has landed. |
| **Our code** | `export!`, factory, process, audio-ports (+ config, sidechain), note-ports, params (sample-accurate + mono mod), state, GUI, remote-controls, latency, tail, render, **preset-load** (+ discovery when factory presets exist) |

Rules:

1. **New extensions and behaviour** come from free-audio docs/headers (product needs decide *which* extensions we ship).
2. Bindings updates or vendoring require an explicit source/revision choice and
   compatibility checks. The current dependency remains crates.io 0.6.0; do not
   describe the maintainer fork or a future in-tree crate as already integrated.
3. **Do not invent** extension IDs or struct layouts — copy free-audio names.

### Version snapshot (check when bumping)

Checked against the dependency lock **2026-11-28**; upstream reference below is
a versioned header baseline, not a claim about today's newest upstream commit.

| | Version |
|--|---------|
| free-audio/clap reference | **1.2.10** |
| clap-sys (this workspace lock) | **0.6.0** — `CLAP_VERSION_*` = **1.2.10** |

Binding availability alone does not imply extension support; see the matrix.

## Usage

```toml
lx-aura = { path = "../AURA/crates/aura", features = ["clap"] }
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

**Param indication:** `PluginLogic::on_param_mapping` and `on_param_automation` run on the main thread. Mirror their flags into atomics shared by `Params` and the editor; read them when drawing UI. Keep this state out of audio DSP and do not take a lock in `process()`.

**Note names:** `PluginLogic::note_names()` returns the table queried by hosts through `clap.note-name`. Use `-1` for wildcard port, channel, or key. `smoke-synth` exposes C4, A4, and C5 on port 0, any channel.

**Note out:** push to `ProcessContext.notes_out`. CLAP emits `NOTE_ON`/`OFF`/`CHOKE`/`END` and expressions. `midi_out` is always raw `CLAP_EVENT_MIDI` (Bitwig Note FX forwards that dialect). `NOTE_END` (voice silent) needs no output note port — Bitwig can drop poly mods. Generated notes (arp / seq) need `PluginInfo.emits_midi` so a note output port exists. VST3/LV2 map On/Off/Choke to 7-bit MIDI.

**Voice info:** set `PluginInfo.voice_count` / `voice_capacity` (`> 0`) to advertise `clap.voice-info` with overlapping notes. Bitwig Voice Stack stays mono-mapped without this.

**Latency:** override `PluginLogic::latency(&state) -> u32` (samples). Via `clap.latency`; mid-run changes request host restart for PDC.

**Tail:** override `PluginLogic::tail_length(&state) -> u32`. Via `clap.tail` (+ VST3 `getTailSamples`).

**Render:** `clap.render` sets `ProcessContext.process_mode` (`Realtime` / `Offline`).

**Preset load:** `clap.preset-load/2` (compat `clap.preset-load.draft/2`). FILE → `PluginLogic::load_preset_from_file` (default: bytes through `load_state_with_context(Preset)`). PLUGIN → `PluginLogic::apply_factory_preset` by `load_key` (Values pairs or Blob via same load hook). Non-empty factory list also exposes `preset-discovery-factory/2` (PLUGIN / factory content).

**Tuning / MTS-ESP:** set `PluginInfo.supports_tuning = true` to advertise `clap.tuning/2`. In `process()`, query `context.tuning.relative_offset(port, channel, key, sample_offset)` for the detune in semitones and `context.tuning.should_play(...)` to honor host-driven note filtering. Host tuning changes split the block sample-accurately; `PluginLogic::tuning_changed` is called when the host reports that the tuning pool changed.

**MIDI 2:** set `PluginInfo.midi_input_dialect = MidiDialect::Midi2` to prefer `CLAP_NOTE_DIALECT_MIDI2` (CLAP + MIDI 1 still advertised). Incoming `CLAP_EVENT_MIDI2` lands on `ProcessContext.ump` as native [`Ump`] packets (per-note pitch bend, SysEx8, Flex included). A 7-bit image is still mirrored to `midi` when `to_midi1` exists. Push generated packets to `ump_out` — CLAP emits `CLAP_EVENT_MIDI2`. Native notes / expressions stay on `MidiDialect::Clap` + `ProcessContext.notes`.

Input priority: native CLAP note events and expressions, then MIDI 2 channel-voice notes, then MIDI 1 channel-voice notes. MIDI2 note on/off populate `ProcessContext.notes` when the host sent no native CLAP note events; when both MIDI 1 and MIDI 2 mirrors exist, MIDI 2 wins. Velocity keeps its 16-bit precision; `port_index` carries the UMP group and `note_id` follows the MIDI 1 key-based fallback. Raw UMP remains available for per-note controllers and pitch bend; `UmpBuffer::iter_per_note_pitch_bend()` filters the latter.

| `midi_input_dialect` | preferred | supported |
| --- | --- | --- |
| `Clap` | CLAP | CLAP + MIDI 1 + MIDI 2 |
| `Midi2` | MIDI 2 | CLAP + MIDI 1 + MIDI 2 |
| `Midi1` | MIDI 1 | CLAP + MIDI 1 |

**Hot reload:** `cargo aura watch --hot` installs `aura-hot` as `Name.clap` and the real plugin as `Name.impl.dll` (or `.so` / `.dylib`). The host keeps the proxy mapped; watch overwrites the impl. Re-add the instance to run the new DSP.

**Remote-controls:** pages of ≤8 params from `ParamInfo.group`. Empty group = no device page. `"Section/Page"` splits on the first `/`. Hidden/readonly never take a hardware slot.

Products can override generated pages with `PluginLogic::remote_control_pages() -> Some(&'static [RemoteControlsPage])`. Give each page a stable `page_id`; list up to eight parameter IDs (extras dropped). `None` keeps group-derived pages, while `Some(&[])` reports zero pages.

**State:** `clap.state` and (opt-in) `clap.state-context/2` call `PluginLogic::save_state_with_context` / `load_state_with_context`. Ordinary `clap.state` uses `StateContext::Project`. Set `state_context_supported() = true` to expose preset/duplicate contexts. Same hooks drive VST3/LV2 project state and factory preset **Blob** loads. Defaults use the shared AURA blob. Main thread. Custom load overrides must snap smoothers themselves.

**Parameter context menu:** set `context_menu_supported() = true`, return `ContextMenuEntry` values from `context_menu_entries(Some(param_id))`, and handle them in `on_context_menu_action`. Callbacks run on the main thread; unsupported host extension stays absent.

**Host track info:** `EditorBridge::track_info()` polls the host's track name and RGBA color on the editor/main thread. It returns `None` when the host extension is absent or its query fails; each field is independently optional.

**Validator:** fresh Windows/debug runs of all five smoke plugins, validator
0.4.2 via `cargo aura validate`: **0 failed** (2026-11-28). Per-plugin pass/skip
counts and limitations: [support matrix](../../CLAP-SUPPORT.md#verification-and-host-evidence).
The earlier four-plugin report dated 2026-08-11 is historical evidence only.

---

## Outstanding (CLAP) — do not forget

The core surface is implemented. Host proof, implementation limits and missing
extensions remain explicit in the support matrix.

### Product-driven (implement only when a plugin needs it)

| Item | Gap / notes | Trigger |
|------|-------------|---------|
| **>1 sidechain / N aux** | one SC in + one aux out max (`BusLayout::with_sidechain` / `with_aux`) | plugin needs more buses than that pair |
| **Rich state hooks** (host blob > flat params) | v2 envelope is params + `#[persist]`. Extra opaque bytes = G5 | preset that is not param+persist |
| **SysEx typed decode** | raw `SysEx8` / Flex already on `ump` | hardware bridge that wants a typed API |

Implemented: poly mod, note expression, MIDI 2, one aux, preset-load. This does
not establish coverage in every host.

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
- [ ] Multi-page `remote-controls` — needs `ParamInfo.group`; smoke-gain has none
- [ ] Non-zero `latency` / mid-block automation — `PluginLogic::latency` + `CHUNKED` exist; prove on a product plug

**aura-host GUI note:** only plugins with `PluginLogic::editor` enable “Open plugin GUI” (`smoke-gain`). `smoke-synth` is headless — button stays disabled. AURA rejects CLAP floating; host embeds on Windows.

### Bitwig session (copy)

1. Rescan CLAP. Insert **AURA Smoke Synth**. Play a triad — three distinct pitches.
2. Open the note inspector. Move Gain / Timbre on one note only.
3. Inspector → Voice Stacking (e.g. 4). Add **Voice Stack** modulator on Gain, spread amount. Stacked copies of one key should be different levels. Re-add the plugin after this rebuild (flags changed).
4. Insert **AURA Smoke MIDI FX** in the **Note FX** chain (not audio FX) *before* the synth. Transpose 12. Synth plays an octave up. Re-add MIDI FX after this rebuild.
5. Bounce the clip offline.

### Spec / bindings (not a ship blocker)

- The maintainer `clap-sys` fork is not yet the workspace dependency.
- Do **not** claim all extensions: thread-pool and surround/ambisonic are absent;
  context-menu and host track-info are implemented with documented limits.

### Explicit non-goals (AURA)

- AU / AAX / VST2  
- Multi-UI toolkits  

Outstanding items above are product-driven — implement when a plugin needs them, not as framework completeness.
