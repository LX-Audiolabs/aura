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
  versions. Review shell and manifest examples as well. These checks are manual
  until Markdown example coverage is automated; `cargo test --doc` alone does
  not compile Markdown files.
- [ ] Check migration instructions against the previous published API and state
  format. Add a focused regression or round-trip test when behavior changes.
- [ ] Run the checks below on the candidate and record results, toolchain, and OS.
  Verify the declared MSRV when changing Rust requirements or dependencies.

```bash
cargo fmt --all -- --check
cargo build --workspace
cargo test --workspace
cargo test --workspace --doc
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
```

Use supported platform/backend configurations; do not enable all mutually
exclusive renderer features together. Record any platform exclusions and the
corresponding check on a supported OS. Current PR CI covers formatting and
Clippy for code changes; it does not replace this release checklist.
Semver automation and Markdown tests remain follow-up work. Run CLAP validation
locally when changing the wrapper or plugin behavior; it is not a default
Forgejo push/PR job.

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
