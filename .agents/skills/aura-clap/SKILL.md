---
name: aura-clap
description: AURA CLAP wrapper specifics — extension coverage, note ports, MIDI 2, state-context, preset-load, tuning, hot reload. Use when editing crates/aura-clap or reasoning about how CLAP maps through AURA and down-converts to VST3/LV2.
---

# AURA CLAP wrapper

`crates/aura-clap` wraps the CLAP C ABI for AURA. It is **AURA's** wrapper —
for the CLAP spec/threading model itself, load the global `clap-review` skill.

## Source of truth

- Spec / ABI: [free-audio/clap](https://github.com/free-audio/clap).
- Rust bindings: `clap-sys` today; in-tree takeover planned — `docs/clap-sys.md`.
- Our code: `export!`, factory, process, audio-ports (+config, sidechain),
  note-ports, params, state, GUI, remote-controls, latency, tail, render,
  preset-load (+discovery), tuning, MIDI 2.
- Full detail: `crates/aura-clap/README.md`.

## Extension coverage (honest matrix)

Done: entry, factory, audio-ports (mono/stereo + optional sidechain), params
(sample-accurate + mono/poly mod), state, GUI, remote-controls, latency, tail,
render, preset-load, tuning (`clap.tuning/2`, MTS-ESP), MIDI 2.

Do **not** claim "full CLAP zoo". Non-goals: AU/AAX/VST2, multi-UI toolkits.
Remaining items are product-driven (see README "Outstanding").

## Key mappings to know

- **Notes in:** native CLAP note events + expressions land on
  `ProcessContext.notes`; a 7-bit image is mirrored to `midi`.
  `MidiDialect` (`Clap`/`Midi2`/`Midi1`) controls the preferred note port.
- **Notes out:** push to `ProcessContext.notes_out` (`NOTE_ON/OFF/CHOKE/END`).
  Generated notes (arp/seq) need `PluginInfo.emits_midi` for a note output port.
- **Poly mod:** `#[param(flags = "modulatable_per_note")]` → per-note `PARAM_MOD`
  on `notes` (`note_id >= 0`); mono mod is `note_id < 0`.
- **MIDI 2:** `PluginInfo.midi_input_dialect = MidiDialect::Midi2` → native UMP
  on `ProcessContext.ump`, 7-bit mirror on `midi`.
- **State:** `clap.state` + opt-in `clap.state-context/2` call
  `save_state_with_context` / `load_state_with_context`
  (`StateContext::{Project, Preset, Duplicate}`).
- **Preset-load:** FILE → `load_preset_from_file`; PLUGIN → `apply_factory_preset`
  by `load_key`; factory list enables `preset-discovery-factory`.
- **Tuning:** `PluginInfo.supports_tuning = true`; query
  `context.tuning.relative_offset(...)` / `.should_play(...)`.
- **Latency/tail:** `PluginLogic::latency` / `tail_length`.
- **Hot reload:** `cargo aura watch --hot` (proxy `.clap` + `Name.impl.*`).

## Down-conversion (VST3/LV2)

`notes_out` and `ump_out` down-convert to 7-bit MIDI On/Off/Choke on VST3/LV2.
LV2 uses only the first bus layout. Write format-agnostic DSP.

## Validation

- `cargo aura doctor`, `clap-validator validate path/to/x.clap`.
- In-tree smokes (`smoke-gain`, `smoke-midi-fx`, `smoke-sidechain`,
  `smoke-synth`) — keep clap-validator green.
