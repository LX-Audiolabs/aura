<!-- AGAL:AUTO-START -->
# smoke-gain

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `member` |
| path | `examples/smoke-gain` |
| description | AURA in-tree CLAP smoke — stereo gain |
| frameworks | aura, aura-editor, slint |
| generated | `2026-08-26T06:01:31Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=member id=examples/smoke-gain
[ATOM] type=fact | detail=frameworks=aura+aura-editor+slint
[ATOM] type=fact | detail=roles=build+entry+manifest+slint
[ATOM] type=fact | detail=has_process=true
[ATOM] type=fact | detail=has_editor=true
[ATOM] type=fact | detail=depends_on=aura
[ATOM] type=fact | detail=depends_on=aura-build
[ATOM] type=fact | detail=depends_on=aura-editor
```

## deps (workspace)
- `aura`
- `aura-build`
- `aura-editor`

## structure
- logic: SmokeGain
- params: GainParams (1 fields)
- process: PluginLogic::process @ src/lib.rs
- editor: yes
- roles: build, entry, manifest, slint

## api surface
- `struct DspState` · `src/lib.rs`
- `struct GainParams { gain: FloatParam }` · `src/lib.rs`
- `struct SmokeGain` · `src/lib.rs`
- `impl PluginLogic for SmokeGain` · `src/lib.rs`

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `examples/smoke-gain` in json / `agal --plugin smoke-gain .`

<!-- AGAL:AUTO-END -->

<!-- AGAL:HUMAN — edit below this line; preserved on regenerate -->

## Intent

_Why this crate/plugin exists. Edit freely._

## Open

- [ ] 

## Decisions

_Architecture choices worth remembering._

## Atoms (human)

_Graph atoms live **above** in AUTO. Add durable decisions/lessons here:_

```text
[ATOM] type=decision|lesson|constraint | detail=…
```
