# Workspace memory

**Summary:** Durable cross-plugin / cross-crate notes for agents.  
**Never overwritten** by `agal .` (unlike per-node AUTO blocks).  
Keep this file short (~80 lines). Prefer `[ATOM]` one-liners.

## Atoms

```text
[ATOM] type=decision|lesson|constraint | detail=…
```

## Open

- [x] G12 sidechain · G13 notes/MIDI · G14 tail/render/preset-load · G17/G18 sample-accurate + mono mod
- [x] LV2 UI · F1 derive split · Stage 6 (watch / mesh / `--hot` / UMP / identity)
- [x] v0.7.1 — no per-block heap in process (Bitwig expression-flood crash)
- [x] v0.7.2 — `notes_out` / `NOTE_END` · `NoteVoiceTable` · native `ProcessContext.ump` / `ump_out`
- [ ] **CLAP product-driven** — multi-out / >1 SC; G5 rich state — only if a plugin needs it (`crates/aura-clap/README.md`)
- [ ] **Bitwig host proofs** — remote-controls, latency, mid-block automation, modulator → `modulatable` (+ per-note), expressions, `NOTE_END` teardown, `notes_out` (arp), offline render
- [ ] **Poly smoke / real synth** — `NoteVoiceTable` + `VoiceManager` (multi-voice); smoothing / expression→knob is DSP, not wrapper
- [ ] **Framework test pass** — CI smokes + host proofs, then crates.io (`publish = false`)

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
[ATOM] type=decision | detail=CLAP leftover = multi-out / G5 / host proofs — see aura-clap README; NoteVoiceTable is the note_id + NOTE_END bookkeeping
[ATOM] type=decision | detail=v0.7.2 shipped notes_out + native ump (2026-08-18); 0.8.0 tag skipped — Stage 6 already in v0.7.1
[ATOM] type=decision | detail=CLAP first: ProcessContext.ump is native MIDI 2; midi is the 7-bit fallback. VST3/LV2 must not shrink the process API
[ATOM] type=decision | detail=NoteVoiceTable (note_id + NOTE_END) is the framework voice bookkeeping; plugin still owns oscillators / envelopes
[ATOM] type=decision | detail=cargo aura watch = rebuild+install poll (no notify dep); preview stays Slint-only
[ATOM] type=decision | detail=cargo aura mesh wraps agal; never a build gate (agal_optional)
[ATOM] type=decision | detail=Host panic fence in aura-core + CLAP/VST3/LV2 process+state
[ATOM] type=decision | detail=aura-test crate: state round-trip + process smoke
[ATOM] type=constraint | detail=PeakMeter/FFT/Spectrum widgets stay product design system; @aura basics incl XYPad only
[ATOM] type=decision | detail=notes_out + NOTE_END are the plugin→host note path (arp/seq + poly-mod teardown); DSP still owns smoothing/routing
[ATOM] type=lesson | detail=CLAP/VST3/LV2 process must not heap-alloc; Bitwig note-expression flood crashed the host (2026-08-18) — scratch reserved in activate, events capped at 4096
[ATOM] type=constraint | detail=No AU/egui zoo; product shm/vault/*Shared stay product
[ATOM] type=decision | detail=JUCE-shaped: aura-dsp (signal) + aura-midi (messages)
[ATOM] type=decision | detail=Portable DSP algos land under aura-dsp modules (docs/dsp-layout.md)
[ATOM] type=decision | detail=ProcessContext.midi / midi_out wired across CLAP/VST3/LV2
[ATOM] type=decision | detail=AURA state = flat param blob (truce-like); no vault/MD/SNAP migration tools in framework core
[ATOM] type=decision | detail=crates.io last — only after framework test pass (CI smokes + host proofs); keep publish = false
```
