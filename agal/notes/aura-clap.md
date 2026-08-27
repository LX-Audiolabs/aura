<!-- AGAL:AUTO-START -->
# aura-clap

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura-clap` |
| description | CLAP format wrapper for AURA — free-audio/clap via clap-sys (minimal v1) |
| frameworks | aura, clap |
| generated | `2026-08-27T16:56:27Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=crate id=crates/aura-clap
[ATOM] type=fact | detail=frameworks=aura+clap
[ATOM] type=fact | detail=roles=entry+manifest+source+state
[ATOM] type=fact | detail=depends_on=aura-core
[ATOM] type=fact | detail=depends_on=aura-params
[ATOM] type=fact | detail=used_by=aura via depends_on
```

## deps (workspace)
- `aura-core`
- `aura-params`

## dependents (inbound)
- `aura` --depends_on--> `aura-clap`

## structure
- public_api symbols: 9 (see json)
- roles: entry, manifest, source, state

## api surface
- `fn entry_deinit()` · `src/lib.rs`
- `fn entry_init(_plugin_path: *const c_char) -> bool` · `src/lib.rs`
- `fn get_factory<L>(factory_id: *const c_char) -> *const c_void` · `src/lib.rs`
- `impl EditorBridge for ClapBridge` · `src/lib.rs`
- … +5 more public symbols

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `crates/aura-clap` in json / `agal --plugin aura-clap .`

<!-- AGAL:AUTO-END -->

<!-- AGAL:HUMAN — edit below this line; preserved on regenerate -->

## Intent

Thin CLAP wrapper around `PluginLogic`. Authors call `aura::export!` — no
hand-rolled `clap_plugin_factory`. Spec source of truth: free-audio/clap headers;
Rust bindings via `clap-sys`. Full extension list + host-proof checklist:
[README.md](../../crates/aura-clap/README.md).

## Open

- [ ] **Product-driven** — multi-out / >1 sidechain; G5 rich state — only if a
      plugin needs it (README)
- [ ] **Bitwig host proofs** — chord, expressions, poly-mod, MIDI FX → synth,
      bounce (README session copy)
- [ ] Typed SysEx/Flex decode — raw packets already on `ump`

## Decisions

- Ship-capable CLAP core is done (ports, params, GUI, notes, UMP, tuning/2,
  preset-load, tail/render). Leftovers are product-driven, not basis holes.
- `is_floating = true` is rejected; AURA GUIs embed only.
- Scratch reserved in `activate`; note/MIDI/UMP events capped at 4096. Plugin
  DSP must still preallocate — the cap is the wrapper flood guard.
- v0.8.0 = `clap.tuning/2` (MTS-ESP). v0.7.2 = `notes_out` + native `ump`.
- `clap-sys` may lag free-audio 1.2.x revision — bump only for a **new**
  extension, not for revision parity.

## Atoms (human)

```text
[ATOM] type=decision | detail=CLAP leftover = multi-out / G5 / host proofs — see aura-clap README; NoteVoiceTable is the note_id + NOTE_END bookkeeping
[ATOM] type=decision | detail=v0.8.0 = clap.tuning/2 (MTS-ESP); tagged 2026-08-20. v0.7.2 = notes_out + native ump (2026-08-18)
[ATOM] type=lesson | detail=CLAP/VST3/LV2 process must not heap-alloc; Bitwig note-expression flood crashed the host (2026-08-18) — scratch reserved in activate, events capped at 4096
[ATOM] type=constraint | detail=aura-clap rejects is_floating=true; AURA plugins are embed-only
```
