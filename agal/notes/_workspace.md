# Workspace memory

**Summary:** Durable AURA framework notes for agents.  
**Never overwritten** by `agal .` (unlike per-node AUTO blocks).  
Keep this file short (~80 lines). Prefer `[ATOM]` one-liners.

## Open

- [ ] **P0** Bitwig GUI open on `examples/smoke-gain` (parented Slint; validator ≠ host) — top basis item
- [ ] **P1** VST3 then LV2 thin wrappers (Stage 5)
- [ ] Product cutover pilot only after basis + G1 ID pass — see `docs/gaps-and-optimizations.md`

## Decisions

```text
[ATOM] type=decision | detail=AURA = Slint + baseview only; formats CLAP/VST3/LV2; no egui/iced/Vizia, no AU/AAX/VST2
[ATOM] type=decision | detail=CLAP-first (Bitwig); free-audio/clap is ABI truth; clap-sys may lag revision
[ATOM] type=decision | detail=Strategy: finish framework basis in-tree (smoke) before product cutover; no early lx-audiolabs-plugins migrate
[ATOM] type=decision | detail=Roadmap: docs/migration-steps.md · gaps/opts: docs/gaps-and-optimizations.md
[ATOM] type=decision | detail=Product DSP/UI catalog stays out: lx-dsp, lx-analysis, lx-shm, lx-vault, lx-ui-slint
[ATOM] type=decision | detail=Thin formats: one PluginLogic API; wrappers never shape core
[ATOM] type=decision | detail=External multi-UI stacks: point to nice-plug (NIH retiring), truce, or clack — not AURA
[ATOM] type=decision | detail=agal optional mesh for authors; never hard-dep for cargo aura build/install
[ATOM] type=decision | detail=UI assets shared via aura_build::materialize_assets (build.rs + aura-preview)
[ATOM] type=decision | detail=aura-derive required for author UX; selective truce port — no plugin_info/State/LV2 sidecars in derive
[ATOM] type=decision | detail=Param IDs explicit id=N only (wire-stable); no silent auto-assign renumber
[ATOM] type=decision | detail=G2 ParamId = option A: derive emits <Struct>ParamId (id()/from_id()); editors use P::X.id(); nested structs get own enum
[ATOM] type=decision | detail=install --clap ships exactly <package>.clap matched by artifact stem; no stray target-dir artifacts
[ATOM] type=constraint | detail=f32 leaf process first; precision-64 / hot-reload only when needed
[ATOM] type=constraint | detail=Audio thread: no alloc/lock in process; param gestures via queue → CLAP out_events
[ATOM] type=lesson | detail=clap-validator green ≠ Bitwig GUI; always host-smoke parented editor before calling GUI done
[ATOM] type=lesson | detail=Product truce plugins often omit param ids and use *ParamId enums — cutover needs ID pass + G2 decision
[ATOM] type=lesson | detail=Scaffold agal.toml is stub only until author opts into full orientation
```

## Deferred (explicit)

```text
[ATOM] type=decision | detail=Stage 6 only after basis: cargo aura init, aura-gui, kind templates, deeper agal mesh
[ATOM] type=decision | detail=MIDI note-ports / multi-bus only when a smoke or pilot needs them
[ATOM] type=decision | detail=Split aura-derive lib.rs modules only if maintainability hurts (F1)
```
