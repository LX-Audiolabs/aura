# AURA DSP / MIDI layout (JUCE-shaped)

**Decision (2026-08-08):** Framework DSP+MIDI follow JUCE module split.

| AURA | JUCE analogue | Role |
|------|---------------|------|
| **`aura-dsp`** | `juce_dsp` (+ synthesis bits of `juce_audio_basics`) | Osc, filters, FX, dynamics, analysis maths, voice engines |
| **`aura-midi`** | `juce_audio_basics/midi` | `MidiMessage`, `MidiBuffer`, note helpers, `Ump` (MIDI 2 stubs) |
| **`aura-core`** | `juce_audio_processors` (thin) | `PluginLogic`, process, buffer, transport, editor trait |
| **`aura-params`** | param/automation pieces | Params + host MIDI-learn *hints* (`MidiSource`) |
| Format crates | format wrappers | CLAP / VST3 / LV2 |

```
crates/
  aura-core/     process API, host fence
  aura-params/   parameters
  aura-midi/     MidiMessage, MidiBuffer, Ump     ← notes / CC / MIDI 2 stubs
  aura-dsp/      all signal math + synth + fx     ← sample path
  aura-clap|vst3|lv2/
  aura-editor/ + aura-baseview/ + aura-build/     UI stack
```

## Inside `aura-dsp` (target tree)

Not every folder exists yet; grow toward this, not a kitchen-sink root `lib.rs`.

```
aura-dsp/src/
  oscillator/   wavetable, polyblep, …
  filter/       biquad, svf, ladder, …
  envelope/     adsr, multistage
  dynamics/     comp, limiter, gate
  effects/      chorus, delay, reverb, distortion
  eq/           parametric, graphic
  analysis/     ✅ portable FFT/SNAP maths, spectrum, meter blocks (no vault/MD/*Shared)
  fx/           ✅ ported from lx-dsp (Biquad TDF-II, mastering, meters, …)
  synth/        higher-level engines (subtractive, FM, …)
  voice/        polyphony / steal
  maths/        denormal, approx, tables           (optional later)
  acoustics/    optional feature
```

## What stays outside AURA (product catalogs)

Product plugin repos own product-only infra. AURA does **not** absorb:

| Keep in product | Why |
|-----------------|-----|
| Multi-plugin shared memory / relay | Product topology, not framework core |
| Vault / MD frontmatter / last-preset paths | AppData layouts differ per brand |
| Product `*Shared` analysis structs | UI/host-facing product types |
| PeakMeter / FFT / Spectrum widgets | Product design system (`@aura` stays basic) |
| Per-plugin preset profiles | Product UX |

Portable **algorithms** stay under **`aura-dsp`**. Vault / MD presets / SNAP-as-file /
product shared state stay in product catalogs.

## Naming

- ~~`aura-synth`~~ → **`aura-dsp`** (covers synth + FX + analysis).
- **`aura-midi`** separate so note/CC path does not drag DSP deps into pure MIDI tools.

## Umbrella

```rust
use aura::dsp::*;
use aura::midi::{MidiBuffer, MidiMessage};
// or from process:
// context.midi.iter()
```

## Process MIDI

`ProcessContext::midi` / `midi_out` — CLAP, VST3, and LV2 wrappers fill and flush note/MIDI events (see in-tree `examples/smoke-midi-fx`).
