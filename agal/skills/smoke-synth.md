---
source: auto-draft
copied_by: agal skill draft
date: 2026-08-27
adapted: false
id: smoke-synth
group: plugins
summary: smoke-synth — MIDI + CLAP notes + transport
triggers: smoke-synth, SmokeSynth, SynthParams
verify: <!-- TODO: add verification checklist -->
---

# smoke-synth

**Summary:** <!-- TODO: describe what this plugin/crate does -->

> Auto-drafted by `agal skill draft`. Fill in the TODO sections.

## Plugin Structure

```rust
impl PluginLogic for SmokeSynth {
    type Params = SynthParams;
    type DspState = DspState; // alloc in init/reset, never in process
    // ...
}
```

## Parameters

| Field | Type | Display |
|---|---|---|
| `gain` | FloatParam | Gain |
| `pan` | FloatParam | Pan |

## Process I/O

```
            ──synthesize──→ [Audio Out]
[MIDI In] (consumed, no MIDI out)
[Notes In] ───────────────→ [Notes Out]
[Transport] (read: play/bpm/pos)
```

| Signal | In | Out |
|---|---|---|
| Audio | — | ✓ |
| Sidechain | — | — |
| MIDI | ✓ | — |
| Notes (CLAP) | ✓ | ✓ |
| Transport | ✓ | — |

## TODO

- [ ] Describe the algorithm / DSP approach
- [ ] Add host compatibility notes
- [ ] Add common failure modes
- [ ] Document transport-sync behavior
