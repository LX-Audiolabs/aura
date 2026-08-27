---
id: vst3
group: formats
summary: AURA VST3 path — thin aura-vst3 wrapper; same PluginLogic; UMP/notes down-converted.
triggers: VST3, vst3, IComponent, kSingleComponent, aura-vst3, .vst3
verify: cargo aura build --vst3; vst3_id stable; process API not shrunk; no alloc in process
source: global
copied_by: template
date: 2026-08-27
adapted: true
reason: "AURA VST3 wrapper — authors do not implement IComponent"
---

# VST3 (AURA)

**Summary:** `aura-vst3` wraps the same `PluginLogic` as CLAP. Plugin authors do
**not** implement `IComponent` / `IEditController`. The wrapper is single-component
(`kSingleComponent`) internally.

## Author path

```toml
aura = { path = "...", features = ["vst3"] }
```

```rust
#[cfg(feature = "vst3")]
aura::export!(MyPlugin);
```

Set a **stable** `PluginInfo.vst3_id` (string → TUID). Host sessions key off it —
do not churn after ship.

```bash
cargo aura build --vst3 -plug <name>
cargo aura install --vst3 -plug <name>
```

## Process API (do not shrink)

`ProcessContext` is CLAP-first:

| Field | VST3 wrapper |
|-------|----------------|
| `midi` / `midi_out` | 7-bit channel voice |
| `ump` / `ump_out` | MIDI 1 lifted to type-0x2 UMP on the way in; `ump_out` down-converted to MIDI |
| `notes` / `notes_out` | On/Off/Choke mapped to 7-bit; CLAP expressions / poly-mod are CLAP-native |

DSP written against `ump` + `notes` still compiles and runs. Do not add a
VST3-shaped `process()` and do not drop fields from `ProcessContext`.

Latency / tail: `PluginLogic::latency` / `tail_length` → `getLatencySamples` /
`getTailSamples`.

## GUI

Same `Editor` trait as CLAP (`AuraSlintEditor`). Embed into the host view
(`IPlugView`). Floating windows are not the AURA path.

## Realtime

Same rule as CLAP: no heap in `process`. Scratch reserved at activation.

## Do not

- Hand-write VST3 vtables / `IComponent` in plugin code
- Change `vst3_id` after a product ships
- Assume Bitwig note-expression / Voice Stack behaviour equals CLAP
  (`ProcessContext.notes` is empty on VST3 today — MIDI path only)
- Use VST3 as an excuse to drop `ump` from the core process API

## See also

- `02-frameworks/aura.md` — one `PluginLogic`
- `03-formats/clap.md` — native notes / UMP / leftover extensions
- `notes/aura-vst3.md`
