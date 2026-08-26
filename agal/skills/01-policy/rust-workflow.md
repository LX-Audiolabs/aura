---
name: rust-workflow
group: 01-policy
triggers: rust, cargo, fmt, clippy, ci, format, lint
description: Rust CI discipline — fmt + clippy after every meaningful edit
verify: cargo fmt --check && cargo clippy -- -D warnings
---

# Rust Workflow — fmt + clippy discipline

## Rule

After any meaningful Rust edit: **fmt first, then clippy**.

```bash
cargo fmt
cargo clippy -- -D warnings
```

Never commit code that fails either. The CI runs both; fail locally before CI fails.

## When to run

| Trigger | Action |
|---------|--------|
| Added or changed any `.rs` file | `cargo fmt` |
| Changed logic (not just docs/comments) | `cargo fmt && cargo clippy -- -D warnings` |
| Before commit | Both, always |
| After `cargo add` / dep bump | `cargo clippy` to catch new lint hits |

## fmt

`cargo fmt` rewrites in place — run it, then stage again if anything changed.

For a single file check without rewriting: `cargo fmt -- --check path/to/file.rs`

Workspace-wide check (what CI runs): `cargo fmt --all -- --check`

## clippy

```bash
# Full workspace, all warnings as errors (mirrors CI):
cargo clippy --workspace -- -D warnings

# One crate only:
cargo clippy -p <crate-name> -- -D warnings

# Including tests:
cargo clippy --workspace --tests -- -D warnings
```

**Suppress only intentional deviations** — `#[allow(clippy::...)]` with a comment explaining why.
Never blanket-suppress a lint to make the build green.

## Common clippy groups in this workspace

```rust
#![allow(clippy::missing_safety_doc)]   // FFI glue files only
#![allow(clippy::cast_possible_truncation)] // deliberate C-integer narrowing
```

Workspace-level lints are in root `Cargo.toml` under `[workspace.lints.clippy]`.
Add crate-level allows only when a lint is structurally impossible to fix (e.g. C ABI shape).

## Fix order

1. `cargo fmt` — always first, clears noise
2. `cargo check` — fast type errors
3. `cargo clippy -- -D warnings` — lints
4. `cargo test` — correctness

## Integration with agal findings

`agal doctor` checks whether clippy and clap-validator are on PATH and prints
the exact command to run. Use `agal findings` to see `[ATOM] type=failure` entries
recorded from past clippy surprises.
