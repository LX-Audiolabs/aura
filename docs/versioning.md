# AURA — versioning (SemVer)

How we number releases. Short, so agents and humans do the same thing.

Last pass: 2026-08-07.

## Current

| | |
|--|--|
| **Scheme** | [Semantic Versioning 2.0](https://semver.org/) |
| **Workspace version** | single number in root `Cargo.toml` → `[workspace.package] version` |
| **Crates** | all `aura-*` + `cargo-aura` / `aura-preview` use `version.workspace = true` |
| **crates.io** | `publish = false` until **Basis fertig** (see [migration-steps.md](./migration-steps.md)) |
| **Git tags** | `vMAJOR.MINOR.PATCH` (annotated), e.g. `v0.1.0` |
| **Changelog** | root [`CHANGELOG.md`](../CHANGELOG.md) ([Keep a Changelog](https://keepachangelog.com/)) |

Examples (`smoke-gain`, baseview demos) may pin their own version for packaging tests; they do **not** define the framework version.

## 0.x vs 1.0

We are in **0.y.z** until the framework is ready for product cutover and a stable author surface.

| Range | Meaning |
|-------|---------|
| **0.y.z** | Public API may change. Prefer bumping **MINOR** for breaking or large feature drops; **PATCH** for fixes/docs. |
| **1.0.0** | First stable line: Basis DoD green + at least one product pilot on AURA; documented compatibility promise. |

Cargo treats `0.1` → `0.2` as a major-style break for dependents. That matches us: path deps today, crates.io later.

## What counts as a “release”

A release is **not** every merge to `main`. A release is:

1. Bump `[workspace.package] version` (one place).
2. Update `CHANGELOG.md`: move `## [Unreleased]` items under `## [X.Y.Z] - YYYY-MM-DD`.
3. Commit: `chore: release vX.Y.Z` (or `release: vX.Y.Z`).
4. Annotated tag: `git tag -a vX.Y.Z -m "vX.Y.Z"`.
5. `git push origin main --tags` (as `lxndrbe`).

Day-to-day features go under **Unreleased** (or just land on `main` and get summarized at tag time).

## Bump guide (0.x)

| Change | Bump |
|--------|------|
| Bugfix, docs, install path, clippy, no API shape change | **PATCH** `0.1.0` → `0.1.1` |
| New format surface, new public trait methods, derive behavior authors rely on, install CLI flags | **MINOR** `0.1.0` → `0.2.0` |
| Intentional hard break of PluginLogic / Params / export macros (document in CHANGELOG) | **MINOR** while 0.x (call it out as **BREAKING**) |
| First stable cut | **1.0.0** |

While path-deps only, dependents are in-tree — still bump so tags/CHANGELOG stay honest.

## Compatibility promise (even in 0.x)

Try hard **not** to break without a MINOR bump:

- `PluginLogic` method signatures
- `Params` / derive `id = N` and emitted `*ParamId`
- `aura::export!` / `export_vst3!` / `export_lv2!` entry symbols
- Flat state blob layout (`aura_core::state`) — wire-stable once a host ships sessions
- `PluginInfo` fields authors set (additive OK; renames = BREAKING)

Wire IDs (`clap_id`, `vst3_id`, `lv2_uri`, param `id`s) are **forever** once a product ships under that ID.

## What we do **not** version separately (yet)

- Per-crate independent SemVer (lockstep workspace is enough pre-crates.io)
- git-cliff / release-plz automation (add when releases hurt by hand)
- Plugin product versions (lx-audiolabs-plugins own their SemVer)

## Checklist (copy into PRs that cut a release)

```text
[ ] CHANGELOG: Unreleased → [X.Y.Z] + date
[ ] Cargo.toml workspace.package.version = X.Y.Z
[ ] cargo test -p aura-core -p aura-clap -p aura-vst3 -p aura-lv2 (smoke optional)
[ ] commit + tag vX.Y.Z + push --tags
```
