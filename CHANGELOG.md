# Changelog

All notable changes to **AURA** (framework workspace) are documented here.

Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [SemVer](https://semver.org/) — see [docs/versioning.md](./docs/versioning.md).

## [Unreleased]

### Added

### Changed

### Fixed

### Docs

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
