# AURA — DSP & Synth Roadmap

Audit bestehender DSP-Bibliotheken mit Bewertung für direkte Nutzung vs. Portierung in eigene Crates. Ziel: Synths und Effekte auf AURA-Basis.

**Entscheidung (2026-08-08):** `naad/rust-old` geforkt → `crates/aura-dsp/` (ex `aura-synth`).  
JUCE-shaped split: **`aura-dsp`** + **`aura-midi`** — siehe [dsp-layout.md](./dsp-layout.md).  
GPL-3.0, flat API, aktiv maintained (als Fork). Erweitert mit Spezial-Algorithmen aus fundsp, infinitedsp-core, synfx-dsp, aetherdsp.

Letzter Audit: 2026-08-08 (2 Runden — Erst-Recherche + Nachrecherche aktiver Crates).  
Siehe auch: [gaps-and-optimizations.md](./gaps-and-optimizations.md) · [migration-steps.md](./migration-steps.md).

---

## Status quo: lx-dsp (product repo)

Bestehendes Crate in `lx-audiolabs-plugins/crates/lx-dsp/`. GPL-3.0-or-later.  
**Port 2026-08-08:** Algorithmen unter `aura_dsp::fx` (siehe [dsp-layout.md](./dsp-layout.md)).  
Product-Repo behält `lx-dsp` vorerst als thin re-export / bis Cutover.

### Was lx-dsp liefert (Effekte ✅)

| Modul | Typ | AURA-ready? |
|---|---|---|
| `Biquad` (TDF-II) | Butterworth LP/HP, Peaking EQ, Low/High Shelf, `magnitude_db()` | ✅ |
| `Filter` trait | generisch über Filter-Implementierungen | ✅ in `DspState` |
| `TiltEq` | Lo + Hi Shelf an Pivot-Frequenz | ✅ |
| `LR2Crossover` | 2nd-order LR, inkl. `process_transparent()` für Multiband | ✅ |
| `Compressor` | Feedforward, Soft-Knee, Envelope-Ballistik | ✅ |
| `TwoBandCompressor` | Crossover + 2 Compressoren | ✅ |
| `MasteringSaturator` | Even/Odd/Mixed Harmonics (Black Box HG-2 Style) | ✅ |
| `MasteringClipper` | Soft-Knee Peak-Shaver | ✅ |
| `MsEq` | 4-Band Mid/Side EQ | ✅ |
| `SweeteningEq` | HPF + LPF + Lo Shelf + Hi Shelf | ✅ |
| `DynamicEq` | Peak-Envelope mit Attack/Release | ✅ |
| `AutoLoudMeter` | LUFS (ebur128), True-Peak | ✅ |
| `MasteringMeter` | Integrated LUFS + LRA + True-Peak Hold | ✅ |
| `RmsMeter` | simpel, kein externes Crate | ✅ |
| `ToleranceTable<N>` | deterministische Kanal-Mikrovarianzen (TMT-Style) | ✅ |
| `FtzDazGuard` | RAII FTZ/DAZ auf x86_64 | ✅ |
| `state_migration` | truce binary state → JSON | ⚠️ Migration |

### Was lx-dsp NICHT liefert (Synths ❌)

- Oszillatoren (Sine, Saw, Square, Triangle, Pulse, Wavetable)
- ADSR / AHDSR Envelope
- Voice-Management / Polyphonie
- MIDI → Frequenz
- LFO
- Noise-Generator (white/pink/brown)
- Unison / Detune
- Anti-Aliasing (PolyBLEP, DSF)

---

## Externe Bibliotheken — Audit Runde 1 (fundsp, synfx-dsp, dasp)

### fundsp v0.23.0

| | |
|---|---|
| **Lizenz** | MIT / Apache-2.0 (GPL-3.0-kompatibel) |
| **Aktualität** | Letzter Commit ~6 Monate, 37 Versionen, 1.2k ★ |
| **Rust** | Stable, `no_std` |

**Stärken:** Vollständige Synth-Toolbox — bandlimited Wavetable-Oszillatoren, PolyBLEP, DSF, Spezial (hammond, organ, pluck). ADSR, LFO, 25+ Filtertypen, Moog Ladder, FDN Reverbs, Waveshaping, FFT Resynthesis.

**Problem:** Monolithisches Graph-DSL (`>>`, `&`, `^`, `|` Operatoren, `typenum` Typ-Level). Inkompatibel mit AURAs `PluginLogic`.

**Fazit:** 🔶 Algorithmen-Fundgrube für Erweiterungen. MIT → GPL-3.0 Port erlaubt.

### synfx-dsp v0.5.6

| | |
|---|---|
| **Lizenz** | GPL-3.0-or-later |
| **Aktualität** | ⚠️ >2 Jahre kein Update, Nightly-Requirement |
| **Rust** | Nightly (`std::simd`) |

**Fazit:** ❌ Nicht als Dep. Referenz für VA-Filter (Ladder, SVF, SallenKey).

### dasp v0.11.0

| | |
|---|---|
| **Lizenz** | MIT / Apache-2.0 |
| **Aktualität** | ⚠️ ~6 Jahre alt |

**Fazit:** 🔶 Konzept-Vorlage (`Sample`/`Frame`-Traits, `envelope::Detector`-Pattern).

---

## Externe Bibliotheken — Audit Runde 2 (nachrecherchiert: aktivere Crates)

> Entdeckt via crates.io-Suche nach "synthesis oscillator" / "dsp audio filter".
> Alle <6 Monate alt oder kürzlich revitalisiert — teils durch AI-unterstützte Entwicklung.

### 🔥 naad v1.2.5 — Gewählt als Fundament

| | |
|---|---|
| **Lizenz** | **GPL-3.0-only** (identisch mit AURA!) |
| **Aktualität** | Letzter Commit **vor 5 Tagen**, 4 Versionen, aktiv maintained |
| **Downloads** | 1.1k total, 433 recent (90d) |
| **Rust** | Stable, min 1.89 |
| **Autor** | Robert MacCracken (auch: AGNOS Kernel, dhvani, svara, shruti) |
| **Architektur** | **Flat Modules** — kein Graph-DSL, keine Framework-Zwänge |

**API-Stil (direkt kompatibel mit AURAs `PluginLogic::process`):**
```rust
use naad::oscillator::{Oscillator, Waveform};
use naad::envelope::Adsr;
use naad::filter::{BiquadFilter, FilterType};

let mut osc = Oscillator::new(Waveform::Sine, 440.0, 44100.0)?;
let mut env = Adsr::new(0.01, 0.1, 0.7, 0.3)?;
let mut filter = BiquadFilter::new(FilterType::LowPass, 44100.0, 2000.0, 0.707)?;
env.gate_on();
let sample = osc.next_sample() * env.next_value(44100.0);
let filtered = filter.process_sample(sample);
```

**Enthaltene Module:**
| Modul | Features |
|---|---|
| `oscillator` | Sine, Saw, Square, Triangle, Pulse — alle PolyBLEP anti-aliased |
| `envelope` | ADSR (linear segments) + MultiStageEnvelope |
| `filter` | BiquadFilter (EQ Cookbook: LP/HP/BP/Notch/Allpass/Shelves/Peak) + StateVariableFilter |
| `wavetable` | Additive Synthesis, linear interpolation, morphing zwischen Tables |
| `modulation` | LFO, FM-Synth, Ring-Modulator |
| `delay` | Fractional Delay-Lines, Feedback Comb-Filter, Allpass-Delays |
| `effects` | Chorus, Flanger, Phaser, Distortion (soft/hard clip, wave fold) |
| `noise` | White (xorshift), Pink (Voss-McCartney), Brown (integrated white) |
| `tuning` | Equal Temperament, Just Intonation, Pythagorean, Custom Tuning Tables, `midi_to_freq` |

**Fazit:** 🔥 **Beste Direkt-Dependency.** Gleiche Lizenz, flache API ohne Framework-Zwang, alle Basis-Bausteine, quicklebendig (Commits täglich). Teil des AGNOS-Ökosystems — Autor baut komplette Audio-Pipeline (naad→dhvani→svara→shruti).

---

### infinitedsp-core v1.2.0 — Top-Erweiterungsquelle

| | |
|---|---|
| **Lizenz** | MIT (GPL-3.0-kompatibel) |
| **Aktualität** | Letzter Commit vor 2 Monaten, 5 Releases, 17 ★, 4 Contributors |
| **Downloads** | 1.3k total, 529 recent (90d) |
| **Rust** | Stable, `no_std`, SIMD via `wide` |

**Was naad NICHT hat, infinitedsp-core aber schon:**
- **Predictive ZDF Ladder Filter** (neben iterativem Moog Ladder) — state-of-the-art
- **TPT/ZDF State Variable Filter** (nicht nur Standard SVF)
- **Formant-basierte Speech Synthesis**
- **Wavetable Oscillator** (naad hat Wavetable als eigenes Modul, infinitedsp als Oscillator-Typ)
- **Brass Physical Model**
- **Spectral Processing** (FFT Pitch Shift, Granular Pitch Shift)
- **`perf-approximations` Feature** — polynomiale sin/tan/log-Approximationen für ±0.2% Error, großer Speedup auf langsameren Targets

**Fazit:** 🔶 **Erweiterungs-Fundgrube.** Spezial-Filter (Predictive Ladder, TPT-SVF), Spectral Processing, Speech Synth. Algorithmen per MIT→GPL-3.0-Port in `lx-synth` einbauen.

---

### aetherdsp-nodes v0.2.4 (Teil von AetherDSP)

| | |
|---|---|
| **Lizenz** | MIT |
| **Aktualität** | Letzter Commit vor ~1 Monat, sehr aktiv (2 ★, aber substanziell) |
| **Architektur** | Teil eines größeren Frameworks (Scheduler, Graph, Arena) |

**Highlights für Extraktion:**
- **Huovilainen Moog Ladder** (physikalisch akkurater als Standard-Moog)
- **Cytomic SVF** (Andrew Simper's Linear Trapezoidal)
- **FormantFilter** — Vowel-Morphing A/E/I/O/U
- **Granular-Synth** — Grain-Size, Density, Pitch-Scatter
- **BLEP-Oszillator** — Alternative Anti-Aliasing-Implementierung
- **17 Tuning-Systeme** — Ethiopian, Arabic, Gamelan, Just Intonation

**Fazit:** 🔍 **Referenz** für Moog-Ladder-, Formant-Filter- und Granular-Implementierungen. Framework zu groß zum Einbinden — Algorithmen extrahieren.

---

### patches-dsp v0.6.1 (Teil von Patches)

| | |
|---|---|
| **Lizenz** | MIT |
| **Aktualität** | Letzter Commit vor 2 Monaten, 1 ★ |

**Fazit:** 🔍 **Referenz** für Poly-Architektur-Patterns und CLAP-Integration. DSL-basiert, nicht direkt nutzbar.

---

## Entscheidung: naad als Fundament + selektive Erweiterungen

```
lx-synth = naad (Basis: Osz, ADSR, Biquad, SVF, LFO, Noise, Tuning, Delay, FX)
         + infinitedsp-core (Predictive Ladder, TPT-SVF, Speech Synth, Spectral)
         + fundsp (Moog Ladder, DSF-Oszillatoren, Pluck, SoftSaw)
         + aetherdsp (Huovilainen Ladder, Formant-Filter, Granular)
```

**Direkte Deps:** `naad` (GPL-3.0, direkt kompatibel)  
**Portierte Algorithmen:** fundsp → MIT→GPL, infinitedsp → MIT→GPL, aetherdsp → MIT→GPL  
**Referenz-only:** synfx-dsp (Nightly), patches (DSL), dasp (veraltet)

---

## Lizenz-Kompatibilität

AURA = GPL-3.0-or-later. lx-dsp = GPL-3.0-or-later. naad = GPL-3.0-only.

| Quelle | Lizenz | → GPL-3.0 Nutzung |
|---|---|---|
| **naad** | **GPL-3.0-only** | ✅ **Direkte Dependency** |
| fundsp | MIT / Apache-2.0 | ✅ Algorithmen portierbar (Attribution) |
| infinitedsp-core | MIT | ✅ Algorithmen portierbar (Attribution) |
| aetherdsp-nodes | MIT | ✅ Algorithmen portierbar (Attribution) |
| synfx-dsp | GPL-3.0-or-later | ✅ Referenz, direkt portierbar |
| dasp | MIT / Apache-2.0 | ✅ Konzept-Referenz |

Attribution in Source-Kommentaren wie synfx-dsp es selbst praktiziert (pro Funktion: Original-Autor, Quelle, Lizenz).

---

## Roadmap: aura-dsp Crate (AURA Framework)

Geforkt aus `naad/rust-old/` nach `crates/aura-dsp/` (rename from `aura-synth`).  
GPL-3.0-or-later, stable Rust. Plugin-Autoren: `use aura::dsp::*` + `use aura::midi::*`.

```
crates/aura-dsp/
  Cargo.toml        ← aura-dsp, workspace Konventionen
  src/
    lib.rs          ← crate root (ehemals naad / aura-synth)
    oscillator/     ← Sine, Saw, Square, Triangle, Pulse (PolyBLEP)
    envelope.rs     ← ADSR + MultiStageEnvelope
    filter.rs       ← BiquadFilter + StateVariableFilter
    modulation.rs   ← LFO, FM-Synth, Ring-Modulator
    noise.rs        ← White, Pink, Brown
    tuning.rs       ← midi_to_freq(), Tuning Tables
    wavetable.rs    ← Additive, Morphing
    delay.rs        ← DelayLine, Comb, Allpass
    effects.rs      ← Chorus, Flanger, Phaser, Distortion
    dynamics.rs     ← Compressor, Limiter
    eq.rs           ← Parametric EQ
    voice.rs        ← Voice-Management
    reverb.rs       ← Reverb
    panning.rs      ← Stereo/Surround Panning
    dsp_util.rs     ← DSP-Hilfsfunktionen
    smoothing.rs    ← Parameter-Smoothing
    mod_matrix.rs   ← Modulations-Matrix
    acoustics/      ← Ambisonics, Binaural, Room, FDN (optional)
    synth/          ← Subtractive Synth, Drum Synth
    error.rs        ← Error-Typen
```

### Direkt nutzbar (aus naad-Fork übernommen)

| Modul | Was | Status |
|---|---|---|
| `oscillator/` | Sine, Saw, Square, Triangle, Pulse (PolyBLEP) | ✅ compiled |
| `envelope.rs` | ADSR + MultiStageEnvelope | ✅ compiled |
| `filter.rs` | BiquadFilter (EQ Cookbook) + StateVariableFilter | ✅ compiled |
| `modulation.rs` | LFO, FM-Synth, Ring-Modulator | ✅ compiled |
| `noise.rs` | White, Pink, Brown | ✅ compiled |
| `tuning.rs` | `midi_to_freq()`, Equal Temp., Just Intonation, Custom Tables | ✅ compiled |
| `wavetable.rs` | Additive Synthesis, Morphing | ✅ compiled |
| `delay.rs` | Fractional Delay, Comb-Filter, Allpass-Delays | ✅ compiled |
| `effects.rs` | Chorus, Flanger, Phaser, Distortion | ✅ compiled |
| `dynamics.rs` | Compressor, Limiter | ✅ compiled |
| `eq.rs` | Parametric EQ | ✅ compiled |
| `voice.rs` | Voice-Management | ✅ compiled |
| `reverb.rs` | Reverb | ✅ compiled |
| `acoustics/` | Ambisonics, Binaural, Room, FDN (feature `acoustics`) | ✅ compiled |
| `synth/` | Subtractive Synth Engine, Drum Synth | ✅ compiled |

### Stage 1: Erweiterungen — Spezial-Oszillatoren & Filter (Ports)

| Prio | Item | Quelle | Aufwand |
|---|---|---|---|
| P1 | `MoogLadder` — 4th-order resonant LPF | fundsp `moog()` | mittel |
| P1 | `Dsfsaw`, `Dsfsquare` — DSF pristine quality | fundsp `dsf_saw()` / `dsf_square()` | mittel |
| P1 | `SoftSaw` — Bandlimited soft saw | fundsp `soft_saw()` | klein |
| P1 | `Pluck` — Karplus-Strong | fundsp `pluck()` | klein |
| P2 | `PredictiveLadder` — ZDF Moog Ladder | infinitedsp-core `PredictiveLadderFilter` | mittel |
| P2 | `TptSvf` — TPT/ZDF State Variable Filter | infinitedsp-core | mittel |
| P2 | `HuovilainenLadder` — physikalisch akkurater Moog | aetherdsp `MoogLadder` | groß |
| P3 | `Hammond`, `Organ` — Spezial-Wavetable-Oszillatoren | fundsp `hammond()` / `organ()` | klein |
| P3 | `FormantFilter` — Vowel-Morphing A/E/I/O/U | aetherdsp `FormantFilter` | mittel |

### Stage 2: Advanced Processing

| Prio | Item | Quelle | Aufwand |
|---|---|---|---|
| P2 | `Unison` — Voice-Stacking mit Detune/Spread | Eigenbau (auf voice.rs) | mittel |
| P2 | `SpeechSynth` — Formant-basierte Sprachsynthese | infinitedsp-core | mittel |
| P3 | `GranularSynth` — Grain-Size, Density, Pitch-Scatter | aetherdsp `Granular` | groß |
| P3 | `SpectralPitchShift` — FFT-basiert | infinitedsp-core | groß |
| P3 | `BrassModel` — Physical Modeling | infinitedsp-core | mittel |

---

## Top-Algorithmen zum Portieren (Priorität)

| Quelle | Algorithmus | Warum |
|---|---|---|
| **naad** | alles | Fundament — Osz, ADSR, Biquad, SVF, LFO, Noise, Tuning, Delay, FX |
| fundsp | `moog()` | Moog Ladder, Industrie-Standard |
| fundsp | `dsf_saw()`, `dsf_square()` | DSF pristine quality, kein PolyBLEP-Approximation |
| fundsp | `soft_saw()` | Bandlimited soft saw |
| fundsp | `pluck()` | Karplus-Strong |
| infinitedsp-core | `PredictiveLadderFilter` | ZDF Moog, state-of-the-art |
| infinitedsp-core | `StateVariableFilter` (TPT/ZDF) | Bessere Tuning-Stabilität als Standard-SVF |
| infinitedsp-core | Speech Synthesizer | Formant-basiert, einzigartig |
| aetherdsp | `MoogLadder` (Huovilainen) | Physikalisch akkurateres Modell |
| aetherdsp | `FormantFilter` | Vowel-Morphing |
| aetherdsp | `Granular` | Grain-Size, Density, Pitch-Scatter |

---

## Architektur-Entscheidungen

| Entscheidung | Begründung |
|---|---|
| **`naad/rust-old` als Fork → `crates/aura-dsp/`** | GPL-3.0 = kompatibel, flat API, 9.4K SLOC Rust. Upstream auf Cyrius migriert → Fork nötig |
| **aura-dsp + aura-midi** (JUCE-shaped) | DSP = Signal; MIDI = Messages/Buffer; nicht alles in einen Crate |
| Spezial-Algorithmen direkt in aura-dsp portieren | Fork = volle Kontrolle; Erweiterungen in passende Module |
| Kein fundsp/synfx-dsp/dasp als Dep | fundsp=Graph-DSL, synfx-dsp=Nightly, dasp=veraltet |
| Sample-Typ `f32` | AURA + naad sind f32-only |
| Keine Allokation in `process()` | Audio-Thread: Pre-allocate in `init()`/`reset()` |

---

## Akzeptanzkriterien

- [x] `crates/aura-dsp/` existiert im AURA Framework (rename from aura-synth)
- [x] `crates/aura-midi/` skeleton (MidiMessage, MidiBuffer)
- [x] `aura_dsp::fx` — voller lx-dsp Port (Biquad, Comp, Mastering, Meters, FTZ)
- [x] `aura_dsp::analysis` — portable lx-analysis (FFT/SNAP, spectrum, blocks; no shm/vault/*Shared)
- [x] `ProcessContext.midi` + CLAP note/MIDI fill
- [x] Product `lx-dsp` / `lx-analysis` façades over aura-dsp
- [x] `cargo check -p aura-dsp -p aura-midi -p aura-core -p aura-clap` grün
- [ ] `cargo test -p aura-dsp` full naad suite
- [ ] VST3/LV2 MIDI → `ProcessContext.midi`
- [ ] Smoke-Plugin: Monophoner Synth mit `Oscillator` + `Adsr` → CLAP, hörbar in Bitwig
- [ ] Polyphoner Smoke: Voice-Stealing via `voice.rs`
- [ ] Mindestens 1 portierter Spezial-Filter (MoogLadder) integriert und getestet
- [ ] Keine Allokation im Audio-Thread (`process()`)
- [ ] Quellen-Attribution pro portierte Funktion
- [ ] naad-Attribution in `Cargo.toml` und `lib.rs` erhalten

---

## Nicht-Ziele (Out of Scope)

- Kein Wavetable-Editor / Sample-Loader in v1
- Kein eigenes FM-Synth-Engine (naad hat `FmSynth`)
- Kein Granular-Synth in v1 (Stage 2)
- Kein Physically-Modeled außer Pluck (Karplus-Strong)
- Keine Integration in `aura-derive` (Params reichen)
- Kein MIDI-File-Player / Sequencer
- Kein eigenes Tuning-System (naad hat Equal Temp., Just Intonation, Pythagorean, Custom Tables)
