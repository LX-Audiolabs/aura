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
  analysis/     ✅ portable FFT/SNAP maths, spectrum, meter blocks (no vault/MD/*Shared)
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
| `lx-vault` | Config paths, MD frontmatter, last-preset hints |
| `lx-analysis` | Product `*Shared` + re-exports of portable `aura_dsp::analysis` |
| `lx-editor-utils::snap` | SNAPSHOT-*.md names, vault scan helpers |
| Per-plugin `presets.rs` | Profile types, MD export/import |
| `lx-ui-slint` PeakMeter/FFT widgets | Product UI |

Portable **algorithms** stay under **`aura-dsp`**. Vault / MD presets / SNAP-as-file /
product `*Shared` stay in **lx-audiolabs-plugins**.

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
| `lx-dsp` | thin re-export of `aura_dsp::fx` (if still present) |
| `lx-analysis` | product `*Shared` + re-export portable `aura_dsp::analysis` + `lx-vault` |
| `lx-vault` | config / frontmatter / vault path helpers |

## Process MIDI

`ProcessContext::midi: MidiBuffer` — CLAP fills from note / MIDI events. VST3/LV2 empty until wired.

See also: [dsp-synth-roadmap.md](./dsp-synth-roadmap.md) · [migration-steps.md](./migration-steps.md).
