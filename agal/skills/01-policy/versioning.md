---
id: versioning
group: policy
summary: SemVer conventions, release checklist, 0.x rules for all LX-Audiolabs projects.
triggers: version bump, release, tag, semver, changelog, crate publish
verify: single workspace version; annotated tag; CHANGELOG updated
---

# Versioning (policy)

**Summary:** How LX-Audiolabs numbers releases — SemVer with 0.x conventions.
Applies to all crates, plugins, and tools.

## Workspace versioning (Rust)

- **Single version** in root `Cargo.toml` → `[workspace.package] version`
- All crates use `version.workspace = true`
- `publish = false` until framework stable; crates.io later

## SemVer rules

| Range | Meaning |
|-------|---------|
| **0.y.z** | Public API may change. Bump **MINOR** for breaking/feature drops; **PATCH** for fixes/docs. |
| **1.0.0** | Stable line — documented compatibility promise. |

Cargo treats `0.1` → `0.2` as major break. Match that expectation.

## Release checklist

A release is **not** every merge. A release is:

1. Bump `[workspace.package] version` (one place).
2. Update `CHANGELOG.md`: move `## [Unreleased]` items under `## [X.Y.Z] - YYYY-MM-DD`.
3. Commit: `chore: release vX.Y.Z`.
4. Annotated tag: `git tag -a vX.Y.Z -m "vX.Y.Z"`.
5. `git push origin main --tags`.

## Git tags

- Format: `vMAJOR.MINOR.PATCH` (annotated)
- Example: `v0.1.0`
- Always push tags with the commit.
