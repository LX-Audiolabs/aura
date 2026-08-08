# Changelog

All notable changes to **AURA** (framework workspace) are documented here.

Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [SemVer](https://semver.org/) — see [docs/versioning.md](./docs/versioning.md).

## [Unreleased]

### Added

- `cargo aura new <name> --vst3 --lv2`: scaffold emits format feature lines + `export_vst3!` / `export_lv2!` (CLAP stays default)
- VST3 Bitwig host smoke green (smoke-gain) — recorded in roadmap status
- LV2 host smoke green (smoke-gain, 2026-08-08) — no LV2 UI by design; roadmap P1 closed

### Changed

### Fixed

- `aura-clap`: `state.load` now requests `clap_host_params.rescan(CLAP_PARAM_RESCAN_VALUES)` after a successful restore — clap-validator's state-reproducibility tests failed without it ("parameter values changed without a rescan request")
- `smoke-gain`: `lv2_uri` aligned with `cargo aura install`'s fallback TTL (`https://lx-audiolabs.com/lv2/smoke-gain`) — mismatched URI breaks LV2 host scanning until the build-time TTL sidecar exists
- `cargo aura install`: resolve target dir via `cargo metadata` — workspace-member plugins (e.g. `examples/smoke-gain`) previously failed with "no build dir target\debug"
- Scaffold builds broke on fresh lockfiles: zune-core 0.5.2 ships empty log macros incompatible with zune-jpeg 0.5.15 (via slint-build) — scaffold now pins `zune-core = "=0.5.1"` until fixed upstream

### Docs

- `licensing-compliance.md` §3.3: VST3 checklist resolved — SDK is MIT since VST 3.8.0 (2025-10); path via `vst3-rs` (MIT/Apache); only Steinberg trademark rules remain

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

[Unreleased]: https://github.com/LX-Audiolabs/aura/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/LX-Audiolabs/aura/releases/tag/v0.1.0
