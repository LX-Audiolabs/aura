<!-- AGAL:AUTO-START -->
# aura-vst3

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura-vst3` |
| description | AURA VST3 format wrapper (thin, over PluginLogic) |
| frameworks | aura, vst3 |
| generated | `2026-08-18T19:22:55Z` |

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
- public_api symbols: 17 (see json)
- roles: entry, manifest, source

## api surface
- `fn plugin_factory<L>() -> *mut IPluginFactory` · `src/lib.rs`
- `fn tuid_bytes(id: &str) -> [u8]` · `src/lib.rs`
- `impl Class for PlugView` · `src/gui.rs`
- `impl EditorBridge for Vst3Bridge` · `src/gui.rs`
- … +13 more public symbols

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `crates/aura-vst3` in json / `agal --plugin aura-vst3 .`

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
