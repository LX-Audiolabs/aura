<!-- AGAL:AUTO-START -->
# aura-shm

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura-shm` |
| description | AURA shared memory — cross-plugin IPC hub with seqlock-protected slots and heartbeat liveness |
| generated | `2026-08-17T17:38:48Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=crate id=crates/aura-shm
[ATOM] type=fact | detail=roles=entry+ipc+manifest
```

## structure
- ipc: relay, seqlock, shm
- public_api symbols: 16 (see json)
- roles: entry, ipc, manifest

## api surface
- `struct RelayHub { _shmem: Shmem, shared: *const HubShared }` · `src/lib.rs`
- `fn display_name(name: &str, slot: u8) -> String` · `src/lib.rs`
- `fn now_ms() -> u64` · `src/lib.rs`
- `fn relay_hub() -> Option<&static RelayHub>` · `src/lib.rs`
- … +12 more public symbols

## findings
- [info] **crate_no_dependents**: aura-shm has no inbound workspace edges — unused or only path-included? · `crates/aura-shm` · fix: wire `aura-shm` as a path dep from a plugin/crate, or remove from workspace

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `crates/aura-shm` in json / `agal --plugin aura-shm .`

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
