<!-- AGAL:AUTO-START -->
# aura-core

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura-core` |
| description | AURA core: process, editor, plugin info (minimal surface) |
| frameworks | aura |
| generated | `2026-08-27T16:56:27Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=crate id=crates/aura-core
[ATOM] type=fact | detail=frameworks=aura
[ATOM] type=fact | detail=roles=audio+entry+manifest+source+state+ui
[ATOM] type=fact | detail=depends_on=aura-midi
[ATOM] type=fact | detail=depends_on=aura-params
[ATOM] type=fact | detail=used_by=aura via depends_on
[ATOM] type=fact | detail=used_by=aura-clap via depends_on
[ATOM] type=fact | detail=used_by=aura-editor via depends_on
```

## deps (workspace)
- `aura-midi`
- `aura-params`

## dependents (inbound)
- `aura` --depends_on--> `aura-core`
- `aura-clap` --depends_on--> `aura-core`
- `aura-editor` --depends_on--> `aura-core`
- `aura-lv2` --depends_on--> `aura-core`
- `aura-test` --depends_on--> `aura-core`
- `aura-vst3` --depends_on--> `aura-core`

## structure
- params: TwoParams (0 fields)
- public_api symbols: 75 (see json)
- roles: audio, entry, manifest, source, state, ui

## api surface
- `trait Editor` · `src/editor.rs`
- `trait EditorBridge` · `src/editor.rs`
- `trait IntoEditor` · `src/editor.rs`
- `trait PluginLogic` · `src/plugin.rs`
- … +71 more public symbols

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `crates/aura-core` in json / `agal --plugin aura-core .`

<!-- AGAL:AUTO-END -->

<!-- AGAL:HUMAN — edit below this line; preserved on regenerate -->

## Intent

One `PluginLogic` + one `ProcessContext` for every format. DSP state is
shell-owned (`type DspState`), not `self`. Format wrappers stay thin.

## Open

- None in this crate. CLAP leftovers / host proofs live in `aura-clap`.

## Decisions

- `ProcessContext.ump` is native MIDI 2; `midi` is the 7-bit fallback. VST3/LV2
  lift MIDI 1 into type-0x2 UMP and down-convert `ump_out` — they must not drop
  fields to “what that format needs”.
- `notes` / `notes_out` are CLAP-shaped (on/off/choke/expression/`NOTE_END`).
  `NOTE_END` is the plugin→host silent-voice signal (poly-mod teardown). Arp/seq
  also needs `PluginInfo.emits_midi` so a note output port exists.
- `NoteVoiceTable` is framework bookkeeping (`note_id` + `NOTE_END`). Plugin DSP
  still owns oscillators / envelopes / smoothing.
- Host panic fence (`host_callback*`, `catch_unwind`) wraps process + state at
  the ABI boundary. Requires `panic = "unwind"`. Does **not** catch Win32
  `wnd_proc` abort on UI teardown — that is `ensure_current` in `aura-baseview`.
- State is a flat param blob (truce-like). No vault/MD/SNAP migration tools here.

## Atoms (human)

```text
[ATOM] type=decision | detail=CLAP first: ProcessContext.ump is native MIDI 2; midi is the 7-bit fallback. VST3/LV2 must not shrink the process API
[ATOM] type=decision | detail=NoteVoiceTable (note_id + NOTE_END) is the framework voice bookkeeping; plugin still owns oscillators / envelopes
[ATOM] type=decision | detail=notes_out + NOTE_END are the plugin→host note path (arp/seq + poly-mod teardown); DSP still owns smoothing/routing
[ATOM] type=decision | detail=ProcessContext.midi / midi_out wired across CLAP/VST3/LV2
[ATOM] type=decision | detail=Host panic fence in aura-core + CLAP/VST3/LV2 process+state
[ATOM] type=decision | detail=AURA state = flat param blob (truce-like); no vault/MD/SNAP migration tools in framework core
```
