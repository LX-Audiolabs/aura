# Workspace memory

**Summary:** Durable cross-plugin / cross-crate notes for agents.  
**Never overwritten** by `agal .` (unlike per-node AUTO blocks).  
Keep this file short (~80 lines). Prefer `[ATOM]` one-liners.

## Atoms

```text
[ATOM] type=decision|lesson|constraint | detail=…
```

## Open

- [ ] CI matrix for all six product plugins (post-cutover order #1)
- [ ] G12 sidechain/multi-bus — only if pilot needs
- [ ] G13 note-ports/MIDI in process — instrument/MIDI FX only
- [ ] G14 tail/render/preset-load — CLAP polish
- [x] F1 split aura-derive lib.rs (~2.6k LOC) — parse/codegen/param_enum/params, 2026-08-10
- [ ] Stage 6 optional: richer agal mesh / hot-reload / MIDI 2.0 stubs

## Decisions

```text
[ATOM] type=decision | detail=Rust toolchain moved nightly → stable 1.97.1 2026-08-10 (rust-toolchain.toml pin); stale incremental cache from the old nightly caused rustc ICEs (STATUS_ACCESS_VIOLATION on fontique/resvg) — `cargo clean` fixed it, not a real regression
[ATOM] type=decision | detail=F1 done 2026-08-10 — aura-derive/src/lib.rs 2719→114 LOC; parse.rs (field/attr parsing), codegen.rs (token builders — `gen` is a reserved keyword), params.rs (Params derive orchestration), param_enum.rs (ParamEnum derive); #[proc_macro_derive] fns stay thin wrappers in lib.rs (crate-root requirement), logic moved to module::expand()
[ATOM] type=decision | detail=CI matrix (six plugins) lives in lx-audiolabs-plugins repo, not AURA — build-linux.yml (GH Actions, workflow_dispatch) + build-local-zip.ps1 (local win+linux via zig cross) already exist there; keeping GH workflow as-is but not extending it (GH CI intentionally not the primary path — local script is); fixed stale `aurum`→`mensor` clapNames key in build-local-zip.ps1, mensor still excluded from its default plugin list on purpose
[ATOM] type=decision | detail=G15 AudioTap landed 2026-08-10 — lock-free SPSC sample ring in aura-params; #[skip] declare, concrete Arc<Params> editor access, no core/derive change
[ATOM] type=decision | detail=Stage 7 pilot + catalog migration done 2026-08-09 — all six plugins on AURA path deps
[ATOM] type=decision | detail=Basis fertig 2026-08-08 — DoD green; cutover gate open
[ATOM] type=decision | detail=Stage 5b P1 done: G9 layouts, G10 remote-controls, G11 latency
[ATOM] type=decision | detail=Stage 6 core tooling done: kinds, add, aura-gui (CLI parity)
[ATOM] type=decision | detail=Host panic fence in aura-core + CLAP/VST3/LV2 process+state (cutover blocker 1)
[ATOM] type=decision | detail=aura-test crate: state round-trip + process smoke (cutover blocker 2 minimal)
[ATOM] type=constraint | detail=PeakMeter/FFT/Spectrum widgets stay lx-ui-slint (product); @aura basics incl XYPad only
[ATOM] type=constraint | detail=No AU/egui zoo; lx-shm/vault/product *Shared stay product
[ATOM] type=decision | detail=JUCE-shaped: aura-dsp (signal) + aura-midi (messages); ex aura-synth
[ATOM] type=decision | detail=Portable lx-dsp/lx-analysis algos land under aura-dsp modules (docs/dsp-layout.md)
[ATOM] type=decision | detail=lx-dsp ported → aura_dsp::fx (2026-08-08); product lx-dsp may thin-reexport later
[ATOM] type=decision | detail=lx-analysis portable → aura_dsp::analysis; *Shared/shm/vault stay product
[ATOM] type=decision | detail=ProcessContext.midi MidiBuffer; CLAP note/MIDI → buffer (VST3/LV2 later)
[ATOM] type=decision | detail=Product lx-dsp/lx-analysis thin façade over aura-dsp (+ product *Shared)
```

