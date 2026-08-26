<!-- AGAL:AUTO-START -->
# aura-host

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura-host` |
| description | Minimal CLAP host — load .clap, params, MIDI in, run audio (Phase 1 CLI) |
| frameworks | clap |
| generated | `2026-08-26T06:01:31Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=crate id=crates/aura-host
[ATOM] type=fact | detail=frameworks=clap
[ATOM] type=fact | detail=roles=entry+manifest+source
[ATOM] type=fact | detail=has_process=true
```

## structure
- process methods (DSP): 1
- public_api symbols: 22 (see json)
- roles: entry, manifest, source

## api surface
- `struct Engine { … }` · `src/audio.rs`
- `struct EvList { items: Vec<EvStorage> }` · `src/events.rs`
- `struct Loader { … }` · `src/loader.rs`
- `struct PluginPtr {  }` · `src/loader.rs`
- … +18 more public symbols

## findings
- [info] **crate_no_dependents**: aura-host has no inbound workspace edges — unused or only path-included? · `crates/aura-host` · fix: wire `aura-host` as a path dep from a plugin/crate, or remove from workspace

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `crates/aura-host` in json / `agal --plugin aura-host .`

<!-- AGAL:AUTO-END -->

<!-- AGAL:HUMAN — edit below this line; preserved on regenerate -->

## Intent

Host side of the CLAP FFI — AURA's answer to `free-audio/clap-host`. Everything
else in the workspace implements the *plugin* side; this crate is the mirror,
and the fast dev loop for testing plugins without a DAW. Plan:
[docs/aura-host-idea.md](../../docs/aura-host-idea.md).

Phase 1 (CLI: load, params, MIDI in, audio) is done. Phase 2 is the Slint shell.

## Open

- [ ] Phase 2: Slint shell — device picker, param sliders, keyboard→notes,
      plugin GUI floating (`gui_create(floating=true)`).
- [ ] Phase 2 needs host `params` (`request_flush`, `rescan`) + `gui`
      extensions, and a Main→Audio param ring (Phase 1 sets params only while
      deactivated, so it has no such ring).
- [ ] No audio *input* path — plugin inputs are fed silence.
- [ ] Only `f32` cpal streams; other sample formats exit with an error.

## Decisions

- Binary, not a library: no inbound workspace edges is expected here, so the
  `crate_no_dependents` finding above is noise for this crate.
- Raw `clap-sys` on the host side rather than reusing `aura-clap` — that crate
  is built around `PluginLogic`, the host has no such trait to hang things on.
- MIDI dialect is decided once from `note-ports.preferred_dialect`, not
  per-event: MIDI dialect → raw `CLAP_EVENT_MIDI`, otherwise notes are
  converted to `CLAP_EVENT_NOTE_ON/OFF` and CC/pitchbend are dropped.

## Atoms (human)

_Graph atoms live **above** in AUTO. Add durable decisions/lessons here:_

```text
[ATOM] type=decision | detail=aura-host is a bin; crate_no_dependents finding is expected
[ATOM] type=decision | detail=host side uses raw clap-sys, not aura-clap (PluginLogic-shaped)
[ATOM] type=constraint | detail=one clap_audio_buffer per declared port, not per channel — smoke-sidechain declares in [2,1]
[ATOM] type=constraint | detail=params set via params.flush only while deactivated; live edits need a Main→Audio ring (Phase 2)
[ATOM] type=lesson | detail=cpal callback must not allocate — buffers are preallocated for MAX_FRAMES=4096 and blocks are chunked
```
