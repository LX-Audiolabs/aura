# Workspace memory

**Summary:** Durable cross-plugin / cross-crate notes for agents.  
**Never overwritten** by `agal .` (unlike per-node AUTO blocks).  
Keep this file short (~80 lines). Prefer `[ATOM]` one-liners.

Crate-local decisions live in `notes/<crate>.md` (scan `[ATOM]` there first).

## Open

- [ ] **CLAP product-driven** — multi-out / >1 SC; G5 rich state — only if a plugin needs it (`notes/aura-clap.md`)
- [x] **Bitwig host proofs** — poly, expressions, poly-mod, MIDI FX → synth, bounce OK (2026-08-28); layout UI soft-skip; sidechain optional (`crates/aura-clap/README.md`)
- [ ] Real-synth DSP — smoothing / expression→knob matrix (not a wrapper hole; product/catalog)
- [ ] **crates.io** — after comfort with CI smokes + host proofs (`publish = false` until then)

## Atoms

```text
[ATOM] type=decision | detail=AURA = plugin framework; AGAL = AI orientation layer; together = JUCE-like workflow with AI, in Rust + Slint
[ATOM] type=decision | detail=Rust toolchain pinned stable 1.97.1 (rust-toolchain.toml); stale incremental cache can cause rustc ICEs — `cargo clean` if needed
[ATOM] type=decision | detail=Product plugin CI lives in product catalogs, not AURA; AURA CI = framework smokes + quality
[ATOM] type=decision | detail=Two product catalogs: lx-audiolabs-dev = internal (Mensor WIP + Zig cross-compile); lx-audiolabs-plugins = official public GitHub repo, ship builds on GitHub Actions, no Mensor
[ATOM] type=constraint | detail=No AU/egui zoo; product shm/vault/*Shared stay product
[ATOM] type=decision | detail=JUCE-shaped: aura-dsp (signal) + aura-midi (messages)
[ATOM] type=decision | detail=Basis fertig 2026-08-08 — DoD green; first-class CLAP path 2026-08-11
[ATOM] type=decision | detail=crates.io last — only after framework test pass (CI smokes + host proofs); keep publish = false
[ATOM] type=decision | detail=Bitwig host proofs core OK 2026-08-28; aura-host stays in AURA (no public rust-clap-host split)
```
