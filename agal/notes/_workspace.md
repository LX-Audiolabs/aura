# Workspace memory

**Summary:** Durable cross-plugin / cross-crate notes for agents.  
**Never overwritten** by `agal .` (unlike per-node AUTO blocks).  
Keep this file short (~80 lines). Prefer `[ATOM]` one-liners.

## Atoms

```text
[ATOM] type=decision|lesson|constraint | detail=…
```

## Open

- [x] G12 sidechain (one optional SC) — done 2026-08-11
- [x] G13 note-ports/MIDI I/O — done 2026-08-11
- [x] G14 tail + render + preset-load — load/discovery 2026-08-17
- [x] G17 sample-accurate + G18 mono mod — done 2026-08-11 (CLAP first-class)
- [ ] **CLAP outstanding (product-driven)** — list: `crates/aura-clap/README.md`
  - [ ] poly mod (`PER_NOTE_ID`) / note expression — instrument pilot
  - [ ] multi-out / >1 sidechain — only if plugin needs
  - [ ] G5 rich state hooks — if host blob > flat params
  - [ ] optional Bitwig host proofs (pages, latency, automation, mod, offline)
- [x] LV2 UI extension — done 2026-08-10 (host smoke pending suitable LV2 host)
- [x] F1 split aura-derive — done 2026-08-10
- [x] Stage 6 author loop (2026-08-17) — `cargo aura watch` · UMP stubs + CLAP MIDI2 ingest · `cargo aura mesh`
- [ ] Stage 6 UI — `aura-gui` / `@aura` identity (after author loop)
- [ ] crates.io publish when ready (`publish = false` today)

## Decisions

```text
[ATOM] type=decision | detail=Rust toolchain pinned stable 1.97.1 (rust-toolchain.toml); stale incremental cache can cause rustc ICEs — `cargo clean` if needed
[ATOM] type=decision | detail=F1 done 2026-08-10 — aura-derive split: parse.rs / codegen.rs / params.rs / param_enum.rs; thin wrappers stay in lib.rs
[ATOM] type=decision | detail=Product plugin CI lives in product catalogs, not AURA; AURA CI = framework smokes + quality
[ATOM] type=decision | detail=Two product catalogs: lx-audiolabs-dev = internal (Mensor WIP + Zig cross-compile); lx-audiolabs-plugins = official public GitHub repo, ship builds on GitHub Actions, no Mensor
[ATOM] type=decision | detail=AURA = plugin framework; AGAL = AI orientation layer; together = JUCE-like workflow with AI, in Rust + Slint
[ATOM] type=decision | detail=LV2 UI extension done 2026-08-10 — lv2ui_descriptor + Editor bridge + idleInterface; TTL UI triples when plugin has editor
[ATOM] type=decision | detail=G15 AudioTap landed 2026-08-10 — lock-free SPSC sample ring in aura-params
[ATOM] type=decision | detail=Basis fertig 2026-08-08 — DoD green; first-class CLAP path 2026-08-11
[ATOM] type=decision | detail=CLAP outstanding = poly-mod / note-expr / multi-out / G5 — see aura-clap README; preset-load landed
[ATOM] type=decision | detail=Process MIDI stays 7-bit MidiMessage; Ump is additive; CLAP_EVENT_MIDI2 down-converts
[ATOM] type=decision | detail=cargo aura watch = rebuild+install poll (no notify dep); preview stays Slint-only
[ATOM] type=decision | detail=cargo aura mesh wraps agal; never a build gate (agal_optional)
[ATOM] type=decision | detail=Host panic fence in aura-core + CLAP/VST3/LV2 process+state
[ATOM] type=decision | detail=aura-test crate: state round-trip + process smoke
[ATOM] type=constraint | detail=PeakMeter/FFT/Spectrum widgets stay product design system; @aura basics incl XYPad only
[ATOM] type=constraint | detail=No AU/egui zoo; product shm/vault/*Shared stay product
[ATOM] type=decision | detail=JUCE-shaped: aura-dsp (signal) + aura-midi (messages)
[ATOM] type=decision | detail=Portable DSP algos land under aura-dsp modules (docs/dsp-layout.md)
[ATOM] type=decision | detail=ProcessContext.midi / midi_out wired across CLAP/VST3/LV2
[ATOM] type=decision | detail=AURA state = flat param blob (truce-like); no vault/MD/SNAP migration tools in framework core
```
