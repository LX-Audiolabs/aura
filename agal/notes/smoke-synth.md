<!-- AGAL:AUTO-START -->
# smoke-synth

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `member` |
| path | `examples/smoke-synth` |
| description | AURA in-tree CLAP smoke — 8-voice synth (NoteVoiceTable + NOTE_END) |
| frameworks | aura |
| generated | `2026-08-26T06:01:31Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=member id=examples/smoke-synth
[ATOM] type=fact | detail=frameworks=aura
[ATOM] type=fact | detail=roles=entry+manifest
[ATOM] type=fact | detail=has_process=true
[ATOM] type=fact | detail=depends_on=aura
```

## deps (workspace)
- `aura`

## structure
- logic: SmokeSynth
- params: SynthParams (2 fields)
- process: PluginLogic::process @ src/lib.rs
- roles: entry, manifest

## api surface
- `struct DspState { … }` · `src/lib.rs`
- `struct SmokeSynth` · `src/lib.rs`
- `struct SynthParams { gain: FloatParam, pan: FloatParam }` · `src/lib.rs`
- `impl PluginLogic for SmokeSynth` · `src/lib.rs`

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `examples/smoke-synth` in json / `agal --plugin smoke-synth .`

<!-- AGAL:AUTO-END -->

<!-- AGAL:HUMAN — edit below this line; preserved on regenerate -->

## Intent

CLAP-first instrument smoke: `ProcessContext.notes` (expressions, poly-mod)
plus MIDI fallback. Headless. Proves the wrapper, not a product synth.

## Open

- [x] Mono Osc + Adsr through CLAP — Bitwig stable after v0.7.1 scratch fix
- [x] `NoteVoiceTable` + `NOTE_END` when envelope idle (v0.7.2)
- [x] 8-voice poly — same table + per-voice osc/env (2026-08-18)
- [ ] Sample-accurate smoothing / expression→knob — later, on a real synth 

## Decisions

_Architecture choices worth remembering._

## Atoms (human)

_Graph atoms live **above** in AUTO. Add durable decisions/lessons here:_

```text
[ATOM] type=decision|lesson|constraint | detail=…
```
