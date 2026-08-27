<!-- AGAL:AUTO-START -->
# aura-vst3

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura-vst3` |
| description | AURA VST3 format wrapper (thin, over PluginLogic) |
| frameworks | aura, vst3 |
| generated | `2026-08-27T17:47:49Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=crate id=crates/aura-vst3
[ATOM] type=fact | detail=frameworks=aura+vst3
[ATOM] type=fact | detail=roles=entry+manifest+source
[ATOM] type=fact | detail=has_process=true
[ATOM] type=fact | detail=depends_on=aura-core
[ATOM] type=fact | detail=depends_on=aura-params
[ATOM] type=fact | detail=used_by=aura via depends_on
```

## deps (workspace)
- `aura-core`
- `aura-params`

## dependents (inbound)
- `aura` --depends_on--> `aura-vst3`

## structure
- process methods (DSP): 1
- public_api symbols: 18 (see json)
- roles: entry, manifest, source

## api surface
- `fn plugin_factory<L>() -> *mut IPluginFactory` · `src/lib.rs`
- `fn tuid_bytes(id: &str) -> [u8]` · `src/lib.rs`
- `impl Class for PlugView` · `src/gui.rs`
- `impl EditorBridge for Vst3Bridge` · `src/gui.rs`
- … +14 more public symbols

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `crates/aura-vst3` in json / `agal --plugin aura-vst3 .`

<!-- AGAL:AUTO-END -->

<!-- AGAL:HUMAN — edit below this line; preserved on regenerate -->

## Intent

Thin VST3 wrapper around the same `PluginLogic`. Authors `aura::export!` — no
hand-written `IComponent`. `vst3_id` is stable once shipped (string → TUID).

## Open

- None as a format hole. CLAP-native notes/expressions stay CLAP-only; VST3
  maps On/Off/Choke + `ump_out` down to 7-bit MIDI.

## Decisions

- Single-component internally. Process API stays CLAP-shaped: `ump` is lifted
  from MIDI 1 (type-0x2) on the way in; never drop the field.
- Same no-alloc / scratch-in-activate rule as CLAP.
- Same `Editor` embed path as CLAP (`AuraSlintEditor`).

## Atoms (human)

```text
[ATOM] type=constraint | detail=VST3 must not shrink ProcessContext — ump is lifted MIDI 1 (type-0x2); notes/expressions stay CLAP-native
[ATOM] type=decision | detail=vst3_id is a stable string→TUID; do not churn after ship
```
