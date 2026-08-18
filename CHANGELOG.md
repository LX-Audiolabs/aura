# Changelog

All notable changes to **AURA** (framework workspace) are documented here.

Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [SemVer](https://semver.org/) — see [docs/versioning.md](./docs/versioning.md).

## [Unreleased]

### Added

- CLAP poly `PARAM_MOD` / per-note `PARAM_VALUE` (`note_id ≥ 0`) and `CLAP_EVENT_NOTE_EXPRESSION` → `ProcessContext.notes`. Mono events still hit `Params`. `MidiDialect::Clap` prefers `CLAP_NOTE_DIALECT_CLAP`. smoke-synth: velocity, volume (Bitwig Gain), timbre (sine→saw), pressure, tuning, per-note Gain.
- `cargo aura watch` — poll `src` / `ui` / manifests and rebuild (+ install). Default format `--clap`. `--no-install` skips the host copy. Install copy retries when a host still maps the binary.
- `cargo aura mesh` — thin wrapper over `agal` (default `agal .`). Optional; builds do not need it.
- `aura-midi::Ump` — MIDI 2.0 Universal MIDI Packet stubs (MIDI 1 CV, MIDI 2 note on/off, per-note pitch bend, `SysEx8`, Flex Data, lossy `to_midi1`).
- CLAP: ingest `CLAP_EVENT_MIDI2` (down-convert to `MidiMessage`). `MidiDialect::Midi2` now prefers the MIDI 2 note-port dialect. smoke-synth advertises MIDI 2 in.
- `aura-hot` CLAP proxy: `cargo aura install --hot` / `watch --hot` writes `Name.clap` (host-mapped) + `Name.impl.*` (replaced on each watch). Re-add the instance to pick up new DSP.
- AURA identity tokens: aurora teal on ink in `@aura` `AuraTheme`; `aura-gui` uses the same chrome (wordmark, card, mesh / hot).

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

[Unreleased]: https://github.com/LX-Audiolabs/aura/compare/v0.7.1...HEAD
[0.7.1]: https://github.com/LX-Audiolabs/aura/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/LX-Audiolabs/aura/compare/v0.6.3...v0.7.0
[0.6.3]: https://github.com/LX-Audiolabs/aura/compare/v0.6.2...v0.6.3
[0.6.2]: https://github.com/LX-Audiolabs/aura/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/LX-Audiolabs/aura/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/LX-Audiolabs/aura/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/LX-Audiolabs/aura/releases/tag/v0.5.0
[0.1.0]: https://github.com/LX-Audiolabs/aura/releases/tag/v0.1.0
