# AURA DSP / MIDI layout (JUCE-shaped)

**Decision (2026-08-08):** Framework DSP+MIDI follow JUCE module split.

| AURA | JUCE analogue | Role |
|------|---------------|------|
| **`aura-dsp`** | `juce_dsp` (+ synthesis bits of `juce_audio_basics`) | Osc, filters, FX, dynamics, analysis maths, voice engines |
| **`aura-midi`** | `juce_audio_basics/midi` | `MidiMessage`, `MidiBuffer`, note helpers |
| **`aura-core`** | `juce_audio_processors` (thin) | `PluginLogic`, process, buffer, transport, editor trait |
| **`aura-params`** | param/automation pieces | Params + host MIDI-learn *hints* (`MidiSource`) |
| Format crates | format wrappers | CLAP / VST3 / LV2 |

```
crates/
  aura-core/     process API, host fence
  aura-params/   parameters
  aura-midi/     MidiMessage, MidiBuffer          ← notes / CC stream
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
  analysis/     ✅ portable lx-analysis (FFT/SNAP, spectrum, meter blocks; no shm/vault)
  fx/           ✅ ported from lx-dsp (Biquad TDF-II, mastering, meters, …)
  synth/        higher-level engines (subtractive, FM, …)
  voice/        polyphony / steal
  maths/        denormal, approx, tables           (optional later)
  acoustics/    optional feature
```

## What stays product (`lx-audiolabs-plugins`)

| Keep in product | Why |
|-----------------|-----|
| `lx-shm` | Multi-plugin relay / shared memory |
| `lx-vault` | Presets, config paths, product profiles |
| Per-plugin `*Shared` UI state (Aether/Mensor/…) | Product composition, not framework |
| `lx-ui-slint` PeakMeter/FFT widgets | Product UI |

Portable **algorithms** from `lx-dsp` / `lx-analysis` → modules under **`aura-dsp`**.  
Product plugins keep thin wrappers until cutover.

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

## Product façade

| Product crate | Implementation |
|---------------|----------------|
| `lx-dsp` | `pub use aura_dsp::fx::*` |
| `lx-analysis` | `aura_dsp::analysis` + `lx-shm`/`lx-vault` re-exports + product `*Shared` |

## Process MIDI

`ProcessContext::midi: MidiBuffer` — CLAP fills from note / MIDI events. VST3/LV2 empty until wired.

See also: [dsp-synth-roadmap.md](./dsp-synth-roadmap.md) · [migration-steps.md](./migration-steps.md).
