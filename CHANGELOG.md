# Changelog

All notable changes to **AURA** (framework workspace) are documented here.

Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [SemVer](https://semver.org/) — see [docs/versioning.md](./docs/versioning.md).

## [Unreleased]

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

[Unreleased]: https://github.com/LX-Audiolabs/aura/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/LX-Audiolabs/aura/releases/tag/v0.5.0
[0.1.0]: https://github.com/LX-Audiolabs/aura/releases/tag/v0.1.0
