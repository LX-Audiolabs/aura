<!-- AGAL:AUTO-START -->
# aura-midi

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura-midi` |
| description | AURA MIDI — messages, buffers, note helpers (JUCE juce_audio_basics/midi analogue) |
| generated | `2026-08-17T17:38:48Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=crate id=crates/aura-midi
[ATOM] type=fact | detail=roles=entry+manifest+source
[ATOM] type=fact | detail=used_by=aura via depends_on
[ATOM] type=fact | detail=used_by=aura-core via depends_on
```

## dependents (inbound)
- `aura` --depends_on--> `aura-midi`
- `aura-core` --depends_on--> `aura-midi`

## structure
- public_api symbols: 8 (see json)
- roles: entry, manifest, source

## api surface
- `struct MidiBuffer { events: Vec<MidiEvent> }` · `src/buffer.rs`
- `struct MidiEvent { sample_offset: u32, message: MidiMessage }` · `src/buffer.rs`
- `struct MidiMessage { status: MidiStatus, channel: u8, data1: u8, data2: u8 }` · `src/message.rs`
- `struct Ump { words: [u32], len: u8 }` · `src/ump.rs`
- … +4 more public symbols

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `crates/aura-midi` in json / `agal --plugin aura-midi .`

<!-- AGAL:AUTO-END -->

<!-- AGAL:HUMAN — edit below this line; preserved on regenerate -->

## Intent

JUCE `MidiMessage` / `MidiBuffer` analogue. Format wrappers translate host
events into these types; `PluginLogic::process` reads `ProcessContext::midi`.

MIDI 2 lives here as [`Ump`](../../crates/aura-midi/src/ump.rs) (Universal MIDI
Packet). Process still sees 7-bit `MidiMessage`. Hosts that send
`CLAP_EVENT_MIDI2` are down-converted in `aura-clap`.

## Open

- [x] `SysEx8` / Flex Data packet stubs (2026-08-17)
- [x] Note-id / expressions live on `ProcessContext.notes` (CLAP); MIDI stays 7-bit
- [ ] Optional hi-res / SysEx typed process path (product-driven)

## Decisions

- Keep `MidiMessage` 3-byte channel voice. Do not grow it into UMP.
- `Ump::to_midi1` on MIDI 2 note-on vel 0 yields vel 1 so MIDI 1 does not treat it as note-off.
- Voice extras (`pitch_bend`, `brightness`) stay in `aura-dsp::voice`, not here.

## Atoms (human)

```text
[ATOM] type=decision | detail=MidiMessage stays MIDI 1; Ump is the MIDI 2 stub
[ATOM] type=constraint | detail=no_std-hostile alloc only in MidiBuffer growth
[ATOM] type=lesson | detail=CLAP_EVENT_MIDI2 data is [u32;4] — Ump::from_words
```
