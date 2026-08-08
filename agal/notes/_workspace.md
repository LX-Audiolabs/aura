# Workspace memory

**Summary:** Durable cross-plugin / cross-crate notes for agents.  
**Never overwritten** by `agal .` (unlike per-node AUTO blocks).  
Keep this file short (~80 lines). Prefer `[ATOM]` one-liners.

## Atoms

```text
[ATOM] type=decision|lesson|constraint | detail=…
```

## Open

- [ ] Stage 7 pilot (simple stereo FX first)
- [ ] Stage 6 optional: richer agal mesh / hot-reload

## Decisions

```text
[ATOM] type=decision | detail=Basis fertig 2026-08-08 — DoD green; cutover gate open
[ATOM] type=decision | detail=Stage 5b P1 done: G9 layouts, G10 remote-controls, G11 latency
[ATOM] type=decision | detail=Stage 6 core tooling done: kinds, add, aura-gui (CLI parity)
[ATOM] type=decision | detail=Host panic fence in aura-core + CLAP/VST3/LV2 process+state (cutover blocker 1)
[ATOM] type=constraint | detail=PeakMeter/FFT/Spectrum stay lx-ui-slint + lx-analysis (product); @aura basics incl XYPad only
[ATOM] type=constraint | detail=No AU/egui zoo; no product DSP into AURA
```

