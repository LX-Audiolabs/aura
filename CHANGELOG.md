# Changelog

All notable changes to **AURA** (framework workspace) are documented here.

Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [SemVer](https://semver.org/) — see [docs/versioning.md](./docs/versioning.md).

## [0.11.0] - 2026-08-29

### Added

- `BusLayout::with_aux` — one optional aux output bus (mirror of sidechain-in).
  `AudioBuffer::{main_output,aux_output,num_main_outputs,num_aux_outputs}`;
  `AudioConfig::aux_output_channels`. Wired through CLAP / VST3 / LV2.
- `smoke-aux` — stereo main + stereo aux smoke (main dry, aux = Send × main).
- `cargo aura preset list|pull` — list factory presets via CLAP discovery; pull a
  key to a v1 state blob (`aura-host --list-presets` / `--pull-preset` / `--out`).

### Fixed

- VST3 `getBusCount` now counts sidechain input and aux output buses (was always
  main-only), matching `getBusInfo` / arrangements.

### Removed

- `tools/aura-gui` — superseded by `cargo aura run` / `aura-host`; tree deleted.

## [0.10.1] - 2026-08-28

### Changed

- `aura-host`: bump `cpal` 0.15 → 0.18.2, `midir` 0.10 → 0.11, `libloading` 0.8 → 0.9.

### Fixed

- `aura-host`: adapt to cpal 0.18 (`device.description()`, `SampleRate` as `u32`,
  `StreamConfig` by value) so `cargo aura run --gui` builds again after the dep bump.
- `aura-preview`: inject package version from the nearest `Cargo.toml`, including
  inherited `version.workspace = true`.

## [0.10.0] - 2026-08-27

### Added

- `aura-host` Phase 1 complete: MIDI input (midir → queue → audio
  thread, dialect picked from `note-ports.preferred_dialect`), `--set
  <id>=<val>` via `params.flush` before `activate()`, `--list-midi` /
  `--midi-in <name>`, and the `clap.log` + `clap.thread-check` host
  extensions.
- `aura-host` Phase 2: `--gui` opens a Slint shell — output-device and
  MIDI-port pickers, param sliders (polling `params.get_value`, no output-event
  tracking needed), PC-keyboard note input, and a button for the plugin's own
  GUI. Device switching tears down and reopens the CLAP activation
  (`audio::Session`) instead of the CLI's block-forever `run()`. Host now also
  answers `clap.gui` and `clap.params`, and drains `request_restart` /
  `request_callback` from a 50 ms main-thread timer.
- `aura-host` Phase 3 (Windows): the plugin-GUI button now embeds the plugin's
  own window as a `WS_CHILD` of ours (`gui.create(is_floating=false)` +
  `gui.set_parent`), falling back to a floating top-level window
  (`plugin_gui.rs`, from Phase 2) only when the plugin doesn't support
  embedding. AURA's own plugins support embedding but not floating — verified
  live against `smoke-gain`/`smoke-synth`: the plugin's `aura-baseview` window
  ends up correctly nested inside our socket `HWND`, and closing tears both
  down cleanly.

### Fixed

- `aura-host` passed one `clap_audio_buffer` with `audio_inputs_count` set to
  the *channel* count — the same port-vs-channel confusion fixed on the plugin
  side in 0.9.6. A plugin with two input ports (`smoke-sidechain`: `in [2, 1]`)
  read past the end of the array. Now one buffer per declared port, on both
  sides. The audio callback also no longer allocates per block.

## [0.9.6] - 2026-08-25

### Fixed

- Windows CI `aura-shm` `cv_hub_isolation`: `relay_hub()` returned `None` and
  panicked. `shared_memory` can leave a `%TEMP%\shared_memory-rs` backing file
  after a failed `create()`, `yield_now` retries were too short, and
  `OnceLock<Option<Hub>>` cached that `None` for the process. Retry create/open
  with a short sleep, delete the leftover file when both fail, cache only a
  live hub.
- Bitwig (Win32 CLAP sandbox) abort when closing the plugin UI: Slint's
  `unregister_item_tree` `expect`s a live OpenGL context from inside
  `WM_DESTROY`. Bitwig tears the parent HWND down before `gui_destroy`, so
  `wglMakeCurrent` fails with `ERROR_INVALID_HANDLE` and the panic aborts
  the sandbox (`0xC000041D`). `ensure_current` now continues without a
  current context on that path (warn once per process); GL objects are
  reclaimed with the WGL context. Reload/re-open still worked — only close
  crashed, all plugins. `gui_destroy` is also fenced with `catch_unwind`.
- Leftover Slint window adapter after a panicked `Component::new()` was
  dropped on the *next* editor open, while a different GL/Skia context was
  current — `clear_next_adapter` now drops it immediately.
- CLAP/VST3 advertised stereo sidechain as extra *ports* (channel count
  used as port count, duplicate port ids). `BusLayout::input_port_count`
  counts main + optional sidechain as ports; Bitwig dry-passthrough routing
  no longer breaks.

## [0.9.5] - 2026-08-24

### Added

- `aura-clap`: note-name and param-indication CLAP extensions.
- `aura-dsp`: AR and AHDSR envelope generators with tests.
- `aura-dsp`: `size_scale` parameter on `MatrixFdn` for room-size delay scaling.

### Changed

- Dependabot: bump `rtrb` 0.3.4 → 0.4.0.

## [0.9.4] - 2026-08-23

### Fixed

- `aura-shm` `open_or_create`: retry `open()` briefly when losing the segment `create()` race instead of caching a transient `None` in the hub's `OnceLock` for the whole process. Under parallel `cargo test --workspace` this made `relay_hub()` return `None` → `cv_hub_isolation` panicked → poisoned `CV_TEST_LOCK` → `PoisonError` cascade into the other CV tests. Also hardens real parallel plugin instances opening the hub concurrently.
- CV tests lock `CV_TEST_LOCK` with `unwrap_or_else(PoisonError::into_inner)` so a single failure no longer poison-cascades into the others.

## [0.9.3] - 2026-08-23

### Fixed

- HiDPI editor scaling on hosts that never call `set_scale` (Bitwig on Win32): content scale stayed at `1.0` while the OS gave the per-monitor child a HiDPI backbuffer, so the FemtoVG viewport under-filled it — the UI rendered into a corner with grey margins. The editor now adopts the real OS DPI reported by baseview on open (host-silent path only), folded with UI zoom. Layout stays fixed at design logical size (uniform Slint scale, no reflow); no-op at 100% and when the host does drive scale.
- VST3: implement `IPlugViewContentScaleSupport` and apply host content scale in `getSize` / `onSize` / `checkSizeConstraint` (Win/Linux; macOS identity).
- Pedantic clippy warnings in `aura-shm` (CV publisher reads) and `aura-preview`.

## [0.9.2] - 2026-08-23

### Changed

- `baseview` `=0.3.0` → `=0.3.2` (Win32 destroy fix, X11 viewability/events; WGL path unchanged). 0.3.1 skipped (Linux `--release` `dbg!` breakage).

### Fixed

- FemtoVG `on_frame`: swallow transient render errors instead of returning `Err` (baseview closes the editor window on `on_frame` failure).

## [0.9.1] - 2026-08-23

### Fixed

- Keep the FemtoVG/WGL context current across frames (no per-frame `make_not_current`).
- Soft-fail when Windows returns OpenGL 1.1 (software loader) instead of a driver context.
- Aggressive early host/child size re-assert on Windows to reduce clipped plugin UIs.

### Changed

- Expose `UiZoom::host_scale`.

## [0.9.0] - 2026-08-22

### Added

- `aura-shm` CV channel: second OS segment `lxaudiolabs_cv_v1` (`CvHub` / `CvSlot`), generic `Hub<S>` shared with relay. Payload is 9 floats (`CV_LOCK`…`CV_RAND`: lock, gate, pitch, bus_a/b, eoc, env, lfo, rand). Analyse relay stays on `lxaudiolabs_lucent_relay_v7`.

## [0.8.0] - 2026-08-20

### Added

- CLAP `clap.tuning/2` (draft) / MTS-ESP host tuning support. Plugins opt in with `PluginInfo.supports_tuning` and query `ProcessContext.tuning.relative_offset(...)` / `should_play(...)` per note. Tuning selection events split the block sample-accurately; `PluginLogic::tuning_changed` is called when the host tuning pool changes.
- `TuningProvider::tuning_count` / `tuning_info` and corresponding `Tuning` accessors expose host tuning metadata (`clap.tuning/2` `get_tuning_count` / `get_info`).
- `aura-preview` control window: screenshot button.

### Changed

- Toolchain auf `stable` gestellt.
- Kleine Clippy-Korrekturen in `smoke-synth`, `aura-lv2` und `aura-gui`.

### Fixed

- Drop unused `dependencies.baseview.version` on `aura-baseview` (`workspace = true` already pins `=0.3.0`). Cargo warned on every `cargo aura` invocation.
- `aura-preview` control: larger status/path text on a dark bar (green/red status kept).
- `cargo aura add` / `add-ui`: register the new crate in workspace `Cargo.toml` `members`. agal / `cargo metadata` only see workspace members, so a scaffold-only plugin was invisible. Added plugin crates no longer emit a nested `[workspace]` table (Cargo: `multiple workspace roots`). `[[plugin]].crate` is the package name (for `-p`), not the members path.

## [0.7.2] - 2026-08-18

### Added

- `ProcessContext.ump` / `ump_out` — native MIDI 2 on the process path. CLAP writes `CLAP_EVENT_MIDI2` unchanged (per-note pitch bend, SysEx8, Flex stay); 7-bit `midi` is still the fallback image. VST3/LV2 lift MIDI 1 into type-0x2 UMP and down-convert `ump_out`.
- `NoteVoiceTable` — voice pool keyed by CLAP `note_id`. `apply` takes inbound notes/expressions; `mark_silent` + `flush_ends` emit `NOTE_END`. `aura-dsp::VoiceManager` stores `note_id` (`note_on_id` / `note_off_id`).
- `ProcessContext.notes_out` + `NoteEventKind::End` (`CLAP_EVENT_NOTE_END`). Plugins push generated notes (arp / seq) or voice-end; CLAP emits native note events, VST3/LV2 map On/Off/Choke to MIDI.

## [0.7.1] - 2026-08-18

### Fixed

- CLAP / VST3 / LV2 `process`: reuse activate-reserved scratch, write host outputs in place, and cap note/MIDI events (4096). Bitwig crashed once CLAP note expressions flooded the audio thread with per-block `Vec` allocs. CLAP `emit_midi_events` now pushes the full note/MIDI event (was header-only — host UB).

## [0.7.0] - 2026-08-17

### Added

- CLAP `clap.preset-load/2` (+ draft compat id): load a v1 state file, or a bundled factory preset by `load_key`. `PluginLogic::factory_presets` / `load_preset_from_file` (defaults keep existing plugins compiling). Non-empty factory list also registers `preset-discovery-factory/2` so hosts can index PLUGIN-location presets. smoke-gain ships Unity / Hot.

### Changed

- Drop `zune-core` exact pins (`aura-gui`, `cargo aura new` scaffold). 0.5.2 is yanked; 0.5.3 is the log-macro fix.
- Crate docs / `frameworks/aura` skill: AURA-owned derive identity (no truce branding on the author surface).

## [0.6.3] - 2026-08-17

### Fixed

- `Cargo.lock`: restore smashed `wayland-backend` dep list (`rustix 1.1.4` / `scoped-tls`) so cargo can parse the lockfile again (CI on `main` was red).

### Changed

- Stay on **`baseview` =0.3.0**. 0.3.1 does not compile Linux `--release` (`dbg!(&visibility_state)` vs `Debug` only under `debug_assertions`). Dependabot ignores 0.3.1; `aura-baseview` repeats the version so later patches are visible.
- `thiserror` 2.0.19 → 2.0.20, `zune-core` =0.5.1 → =0.5.3 (Dependabot #5 / #6).
- `aura-dsp::analysis`: removed product **vault** (MD frontmatter / `config.json` / AppData paths) and **`product_shared`** types. Those belong in product catalogs, not the framework. Portable SNAP FFT / meters / spectrum stay in AURA.
- README: public-facing quick start, status, workspace map; product plugins clarified as separate private catalog.
- Docs: hide internal DSP roadmap; drop broken links to private planning docs; rephrase product-boundary sections without private repo paths.

## [0.6.2] - 2026-08-11

### Added

- `aura-dsp` oscillator ports (fundsp MIT/Apache, in-tree): `DsfSaw` / `DsfSquare`, `SoftSaw` (`1/n²` table), `Pluck` (Karplus–Strong).
- `aura-dsp` filter ports: `filter::MoogLadder` (fundsp), `filter::PredictiveLadder` (InfiniteDSP ZDF).

### Fixed

- macOS: `cargo aura install --clap` ships a CFBundle (`Contents/MacOS/` + `Info.plist`) so clap-validator and hosts can load the plugin (was a flat Mach-O → "Could not open bundle").
- DSF matches fundsp `Dsf::tick` (spacing `d`, Nyquist-limited `n`, no bogus `(1−r)` gain).
- Pluck `set_frequency` recomputes gain from stored `gain_per_second`; delay uses `sr/freq − 1`.

### Changed

- One Moog only: dropped RK4 ladder from `synth::physical` (use `filter::MoogLadder` / `PredictiveLadder`).
- `physical` module is Karplus–Strong + waveguide only.

## [0.6.1] - 2026-08-11

### Changed

- **Ship matrix** (documented support / CI install targets):
  - **CLAP** — Linux, Windows, macOS (primary format)
  - **VST3** — Windows, macOS only (no Linux/Wine ship path)
  - **LV2** — Linux only
- Framework CI installs only the matrix formats per OS; Quality excludes `aura-lv2` off Linux.
- `cargo aura help` / `doctor` print the ship matrix.
- README: **CLAP first** section parallel to **Slint only** (motto + matrix table).

### Fixed

- Linux CI: install `libfontconfig1-dev` / related deps so cold-cache Slint-backed builds work.
- Windows CI: clap-validator path via `$RUNNER_TEMP` (unquoted `D:\a\_temp` was eaten by bash).
- macOS: `aura-editor` AppKit parent handle uses `NonNull` (rwh 0.6; `NonZero<*mut c_void>` does not compile).
- Workspace `rustfmt` clean so Quality `cargo fmt --check` passes.

## [0.6.0] - 2026-08-11

### Added

- `ProcessContext::midi_out` + `with_midi_out()` / `clear_midi()` — plugins can generate MIDI events.
- MIDI output flushed to the host by `aura-clap` (note-ports), `aura-vst3` (event output bus), and `aura-lv2` (MIDI atom output port).
- `BusLayout::sidechain_in` + `with_sidechain()` — one optional mono/stereo sidechain input bus per layout.
- `AudioBuffer` channel ordering `[main_in…] [sidechain_in…] [main_out…]` with `main_input()`, `sidechain_input()`, `num_main_inputs()`, `num_sidechain_inputs()`.
- `AudioConfig::sidechain_input_channels` + `with_sidechain_channels()`.
- `examples/smoke-midi-fx`: MIDI thru + transpose; clap-validator green (33 passed, 0 failed).
- `examples/smoke-sidechain`: stereo main + mono sidechain mix; clap-validator green (31 passed, 0 failed).
- **Sample-accurate automation (G17):** `aura_core::chunked_process` + CLAP process sub-block splits for `ParamFlags::CHUNKED` (default; opt out with `#[param(chunk = false)]`).
- **Mono param modulation (G18):** `Params::set_mod` / `FloatParam` mod amount; `PARAM_MOD` + `CLAP_PARAM_IS_MODULATABLE` flags; DSP effective = `clamp(base + mod)`.
- **`clap.tail` / `PluginLogic::tail_length`** + VST3 `getTailSamples` (G14 partial).
  `host_tail.changed` is invoked on the **audio thread** when active (CLAP thread rule).
- **`clap.render`** → `ProcessContext.process_mode` Realtime/Offline (G14 partial).

### Changed

- Workspace version bumped to `0.6.0` (public `BusLayout`/`AudioConfig` fields extended).
- CLAP process applies non-chunked param events at block start; chunked events at sample-accurate boundaries.

## [0.5.0] - 2026-08-09

### Added

- `cargo aura add-ui <name>`: scaffold a shared Slint UI crate under `crates/<name>/` — minimal theme + barrel, ready for custom components (like `lx-ui-slint`)
- `cargo aura build|install -plug <crate> [<crate>...]`: multi-plugin selector — builds/installs each named workspace member in turn
- `cargo aura install`: artifact now named after the plugin display name (from `aura.toml [[plugin]].name`) instead of the crate name

### Changed

- Workspace version bumped to `0.5.0`; retroactive SemVer changelog entries added for `0.1.0`–`0.4.0` (see [docs/versioning.md](./docs/versioning.md)).

### Product cutover

- lx-audiolabs-plugins catalog fully migrated to AURA: aether, equilibrium, lucent, lucent-relay, mensor, and meridian build, install, and host-smoke via `cargo aura` (CLAP release). The parallel truce framework path is deprecated for LX products. (mensor was renamed from aurum to avoid confusion with AURA.)

## [0.4.0] - 2026-08-08

### Added

- `aura-shm`: shared-memory ring-buffer crate for IPC between audio and UI processes
- `aura-editor`: typed `AuraSlintEditor` layer — plugins construct editors through a high-level builder instead of raw `aura-baseview`
- `aura-dsp` expanded: delay, dynamics, envelope, oscillator (core/sub/sync/unison), reverb, smoothing, wavetable, synth modules (additive, drum, FM, formant, granular, physical, subtractive, vocoder), acoustics, analysis, modulation, noise, tuning
- `aura-midi`: MIDI input routing to `ProcessContext` (CLAP, VST3, LV2)
- `smoke-synth`: example polyphonic synthesizer plugin demonstrating `aura-midi` + `aura-dsp`
- VST3 Bitwig host smoke green (smoke-gain) — recorded in roadmap status
- LV2 host smoke green (smoke-gain, 2026-08-08) — no LV2 UI by design; roadmap P1 closed

## [0.3.0] - 2026-08-08

### Added

- `aura-dsp`: new crate — oscillator, filter, EQ, dynamics, and effects primitives
- `aura-midi`: new crate — MIDI event types and `ProcessContext.midi` channel
- `aura-test`: new crate — state round-trip and process smoke-test helpers
- `aura-gui`: Slint project console (`cargo aura gui` → `tools/aura-gui`)
- `#[skip]` fields in `#[derive(Params)]` for product shared state (e.g. `Arc<SharedMeters>`)
- Host-boundary panic fence across CLAP, VST3, and LV2 wrappers

### Fixed

- `aura-params`: parse kHz/ms/pan text for CLAP `text_to_value`

## [0.2.0] - 2026-08-07

### Added

- `cargo aura new <name> --vst3 --lv2`: scaffold emits format feature lines + `export_vst3!` / `export_lv2!` (CLAP stays default)
- `cargo aura init [path]`: scaffold into an existing empty directory (name from dir, refuses overwrite)
- `cargo aura add <name>`: add a second plugin under `plugins/<name>/` and append `[[plugin]]` to `aura.toml`
- `cargo aura doctor`: probes toolchain, AURA path, clap-validator, and `agal` (info only)
- `new`/`init` accept `--kind <effect|effect-mono|analyzer>` — template surface for future kinds
- Shared scaffold engine (`tools/cargo-aura/src/scaffold.rs`, pure `files()` + unit tests) shared by `new`, `init`, and `add`
- `clap.remote-controls` populated from `ParamInfo.group`
- Mono/stereo bus layouts across CLAP, VST3, and LV2 (`bus_layouts()`)
- Plugin latency reporting for CLAP and VST3 PDC

### Fixed

- `aura-clap`: `state.load` now requests `clap_host_params.rescan(CLAP_PARAM_RESCAN_VALUES)` after a successful restore — clap-validator's state-reproducibility tests failed without it
- `smoke-gain`: `lv2_uri` aligned with `cargo aura install`'s fallback TTL
- `cargo aura install`: resolve target dir via `cargo metadata` — workspace-member plugins previously failed with "no build dir target\debug"
- Scaffold builds broke on fresh lockfiles: zune-core 0.5.2 ships empty log macros incompatible with zune-jpeg 0.5.15 (via slint-build) — scaffold now pins `zune-core = "=0.5.1"` until fixed upstream

### Docs

- `licensing-compliance.md` §3.3: VST3 checklist resolved — SDK is MIT since VST 3.8.0 (2025-10)

## [0.1.0] - 2026-08-07

First versioned baseline. Workspace was already `0.1.0` without tags/changelog;
this release records what `main` contains after the multi-format + UI pass.

### Added

- Framework core: `PluginLogic`, params surface, `aura-derive` (`Params` / `ParamEnum` + `*ParamId`)
- **CLAP** wrapper: factory, stereo, params, process, state, parented GUI (`clap.gui`)
- **VST3** wrapper: factory, process/params/state, `IPlugView` parented GUI, `.vst3` bundle install
- **LV2** wrapper: stereo + control ports, state blob, TTL + `.lv2` bundle install (no UI yet)
- Shared host-agnostic param state codec (`aura_core::state`)
- UI stack: `aura-baseview`, `aura-editor`, `aura-build` (`@aura` + Material 3–aligned `AuraTheme`)
- Tooling: `cargo aura` (`new` / `build` / `install` / `doctor` / `preview`)
- `aura.toml` `[install]` dir with env expansion (`%LOCALAPPDATA%`, etc.)
- In-tree **smoke-gain** example (CLAP Bitwig host smoke green)

### Notes

- Still open for “Basis fertig”: VST3/LV2 real-host smoke; optional LV2 UI; product cutover later
- Crates not published to crates.io (`publish = false`)

[Unreleased]: https://github.com/LX-Audiolabs/aura/compare/v0.7.2...HEAD
[0.7.2]: https://github.com/LX-Audiolabs/aura/compare/v0.7.1...v0.7.2
[0.7.1]: https://github.com/LX-Audiolabs/aura/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/LX-Audiolabs/aura/compare/v0.6.3...v0.7.0
[0.6.3]: https://github.com/LX-Audiolabs/aura/compare/v0.6.2...v0.6.3
[0.6.2]: https://github.com/LX-Audiolabs/aura/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/LX-Audiolabs/aura/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/LX-Audiolabs/aura/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/LX-Audiolabs/aura/releases/tag/v0.5.0
[0.1.0]: https://github.com/LX-Audiolabs/aura/releases/tag/v0.1.0
