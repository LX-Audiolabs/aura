# lx-aura-test

Test helpers for [AURA](https://github.com/LX-Audiolabs/aura) plugins: state
round-trip, param shape checks, and a small offline `process` harness.

**Intended as a `[dev-dependency]`** — not part of the plugin runtime.

```toml
[dev-dependencies]
lx-aura-test = "0.12"
```

```rust
#[test]
fn state_round_trips() {
    aura_test::assert_state_round_trip::<MyPlugin>();
}
```

Lib name stays `aura_test` (`use aura_test::…`). Package name is `lx-aura-test`
(crates.io namespace; same pattern as the rest of AURA).

License: GPL-3.0-or-later.
