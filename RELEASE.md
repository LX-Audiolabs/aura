# Release checklist

Use this checklist for each AURA release. Record the target version, release
commit, and check results in the release PR. Unchecked items keep the release open;
if a check does not apply, record why.

## Prepare the candidate

- [ ] Choose the SemVer version from the changes since the last published release.
  Root `Cargo.toml` (`[workspace.package].version`) is the version source of truth.
  Keep internal workspace dependency versions and `Cargo.lock` consistent.
- [ ] Move the changes being released from `[Unreleased]` into a dated version
  entry in [CHANGELOG.md](CHANGELOG.md). Leave later changes under `[Unreleased]`.
- [ ] Include migration notes in that version's changelog entry: affected APIs,
  before/after usage, parameter IDs, saved-state compatibility, and audible DSP
  changes where relevant. State explicitly when no migration is required.
  Never renumber shipped parameter IDs or silently break saved sessions.

## Verify documentation against the candidate

- [ ] Review [README.md](README.md) and [CLAP-GUIDE.md](CLAP-GUIDE.md) together.
  Check parameter macros, `PluginLogic` signatures, feature names, package names,
  CLI commands, and local links against the release source.
- [ ] Update their API baseline, checkout version, and verification date accurately.
  A candidate must not be described as published before crates.io confirms it.
  Keep published examples and versioned docs.rs links on the published API;
  clearly label any upcoming API separately.
- [ ] Compile the Rust samples in both pages against their stated dependency
  versions. `cargo test --locked -p aura-docs` compiles README and CLAP-GUIDE Rust
  fences against the checkout and checks example consistency. Verify published
  dependency versions separately; review shell/manifest examples and other
  Markdown manually. Workspace doc-tests alone do not cover Markdown files.
- [ ] Check migration instructions against the previous published API and state
  format. Add a focused regression or round-trip test when behavior changes.
- [ ] Run the [manual Forgejo semver workflow](.forgejo/workflows/semver.yml) on
  the candidate commit, or run the command below locally. Record baseline versions
  from its output. Resolve unintended API breaks; intentional breaks require the
  appropriate version bump and migration notes, not suppressed checks.
- [ ] Run the checks below on the candidate and record results, toolchain, and OS.
  Verify the declared MSRV when changing Rust requirements or dependencies.

```bash
cargo fmt --all -- --check
cargo build --workspace
cargo test --workspace
cargo test --workspace --doc
cargo test --locked -p aura-docs
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
```

Use supported platform/backend configurations; do not enable all mutually
exclusive renderer features together. Record any platform exclusions and the
corresponding check on a supported OS. PR CI checks README/guide Rust examples
on every PR, plus formatting, Clippy, and workspace doc-tests for code changes;
it does not replace this release checklist. Other Markdown coverage remains
follow-up work. Run CLAP validation
locally when changing the wrapper or plugin behavior; it is not a default
Forgejo push/PR job.

### API compatibility check

The manual `semver` workflow uses `cargo-semver-checks` **0.51.0** and compares
publishable workspace libraries against their latest eligible crates.io versions.
It does not run on normal PRs or tag pushes. Check the same candidate SHA before
release; a local pass does not imply a Forgejo run has happened.

```bash
cargo semver-checks --workspace --default-features
# Reproduce a comparison against a specific shared release:
cargo semver-checks --workspace --default-features --baseline-version 0.14.6
```

Use `cargo install cargo-semver-checks --version 0.51.0 --locked` if needed.
Update the tool pin deliberately with Rust toolchain upgrades: rustdoc JSON
compatibility depends on the tool version. See the
[tool documentation](https://github.com/obi1kenobi/cargo-semver-checks).

Default features avoid mutually exclusive renderer backends. This checks the
default FemtoVG-GL UI path, not wgpu/Skia/software, and the umbrella's default DSP
surface, not its optional `clap`/`vst3`/`lv2` re-exports. Publishable format crates
are checked directly; `publish = false` crates and examples are excluded by the
tool. This is a Rust API check, not a test of sound, saved states, host behavior,
MSRV, or every possible API break. For pre-1.0 crates, an incompatible change
requires a minor version bump (`0.14.x` to `0.15.0`).

Initial local comparison (2026-11-28, Windows x86_64, Rust 1.99.1, tool 0.51.0):
12 library crates checked against 0.14.6; 11 passed, `lx-aura-dsp` failed two
lint categories. This is not a release pass. The derive proc-macro's expansion
behavior and non-default features need separate tests.

- [x] Restore `DspError` variant-order compatibility: append
  `InvalidFilterParameters` after all five published variants. Fixed bincode
  fixtures test encoding and decoding against 0.14.6's ordering.
- [x] Accept the intentional `FtzDazGuard` type relocation and loss of
  `Send`/`Sync`; retain the shared thread-bound core guard and its DSP re-export.
  The workspace now targets **0.15.0 development**, with migration instructions
  in [CHANGELOG.md](CHANGELOG.md). Do not restore cross-thread movement to
  satisfy a compatibility check.

Follow-up verification for 0.15.0 (same local environment): workspace build,
tests including doc-tests, Clippy `--all-targets -- -D warnings`, and the
README/guide consumer sample passed. The normal workspace semver run against
0.14.6 accepts the version bump and skips breaking-change lints. A stricter DSP
run with `--release-type minor` executes 202 checks: 201 pass, with only the
accepted guard `struct_missing` report remaining. The enum-order report is gone.

No lints are suppressed. The Forgejo workflow has not yet run; its runner
installs the pinned tool if absent or at a different version.

## Publish and verify

- [ ] Confirm the release commit contains the intended code, docs, migration
  notes, and changelog. Complete the checks before publication.
- [ ] Publish the intended publishable crates in dependency order and verify
  the target versions on crates.io. Do not publish path-only crates or examples.
- [ ] Create an annotated `vX.Y.Z` tag on the verified release commit and push
  the release branch and tag to Forgejo (`origin`). GitHub synchronization uses
  the Forgejo mirror; do not push to GitHub directly from this checkout.
- [ ] Verify the Forgejo tag workflow and attached CLI artifact. That workflow
  builds `cargo-aura`; it does not publish the framework crates to crates.io.
- [ ] Verify versioned docs.rs pages and a clean consumer build from crates.io.
  Update README and CLAP-GUIDE published-version statements together once the
  publication is confirmed, and recheck examples if the baseline changes.
  If a follow-up docs commit is necessary, keep it distinct from the release tag;
  do not move an already published tag.
