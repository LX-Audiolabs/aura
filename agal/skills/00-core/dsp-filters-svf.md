---
id: dsp-filters-svf
group: core
summary: BiquadFilter vs StateVariableFilter — wann was, Cytomic SVF, simultane Outputs.
triggers: SVF, state variable filter, biquad, filter, resonance, highpass, lowpass, bandpass, notch, simultaneous, dsp
verify: SR-Wechsel → set_params() aufrufen; reset() nach Transport-Stop; q > 0 prüfen
source: global
copied_by: template
date: 2026-08-25
adapted: true
reason: "aura_dsp::filter hat BiquadFilter (Cookbook) + StateVariableFilter (Simper/Cytomic)"
---

# BiquadFilter vs StateVariableFilter (SVF)

**Summary:** AURA hat zwei kanonische Filter-Typen: `BiquadFilter` (Audio-EQ-Cookbook,
TDF-II) für Standard-Anwendungen, und `StateVariableFilter` (Cytomic/Simper SVF)
für simultane LP/HP/BP/Notch-Outputs und hohe Resonanz.

## 1. Wann welcher Filter?

| Kriterium | `BiquadFilter` | `StateVariableFilter` |
|-----------|---------------|----------------------|
| Standard LP/HP/BP/Notch/Shelf/Peak | ✓ | ✓ |
| Simultane Outputs (LP+HP+BP+Notch) | ✗ | ✓ |
| Hohe Resonanz (Q >> 1) stabil | bedingt | ✓ (Cytomic-Design) |
| Shelf / Peak-Filter | ✓ | ✗ |
| Morph zwischen Filter-Typen | ✗ | ✓ |
| Geringerer Rechenaufwand | ✓ | ✓ (ähnlich) |

**Faustregel:** EQ-Bands → `BiquadFilter`. Synthesizer-Filter / Morph-Designs → `StateVariableFilter`.

## 2. `BiquadFilter` — Audio EQ Cookbook, TDF-II

```rust
use aura_dsp::filter::{BiquadFilter, FilterType};

// Konstruktor gibt Result zurück — SR/freq/Q validiert:
let mut filter = BiquadFilter::new(
    FilterType::LowPass,
    sample_rate,  // > 0.0
    cutoff_hz,    // 0 < freq < sr/2
    q,            // q > 0.0 (Butterworth = 0.707)
)?;

// Mit Gain (nur für Shelf/Peak sinnvoll):
let mut shelf = BiquadFilter::with_gain(FilterType::LowShelf, sr, 200.0, 0.707, 6.0)?;

// Per-Sample (inline): flush_denormal intern auf z1/z2
let out = filter.process_sample(input);

// Per-Buffer (SIMD-freundlich):
filter.process_buffer(&mut buffer);

// Parameter ändern (recomputes coefficients):
filter.set_params(new_cutoff, new_q, new_gain_db)?;

// Reset nach Transport-Stop / SR-Wechsel:
filter.reset();  // löscht z1/z2, lässt Koeffizienten
```

**Verfügbare FilterType-Varianten:**
`LowPass`, `HighPass`, `BandPass`, `Notch`, `AllPass`, `LowShelf`, `HighShelf`, `Peak`

## 3. `StateVariableFilter` — Cytomic/Simper SVF

```rust
use aura_dsp::filter::StateVariableFilter;

let mut svf = StateVariableFilter::new(cutoff_hz, q, sample_rate)?;

// Simultane Outputs in einem Aufruf:
let out = svf.process_sample(input);
let lp = out.low_pass;
let hp = out.high_pass;
let bp = out.band_pass;
let notch = out.notch;

// Convenience: nur LP-Output (intern trotzdem alles berechnet):
let lp_only = svf.process_sample_lowpass(input);

// Buffer-Verarbeitung (LP in-place):
svf.process_buffer_lowpass(&mut buffer);

// Parameter ändern:
svf.set_params(new_cutoff, new_q)?;

// Reset:
svf.reset();  // ic1eq = ic2eq = 0.0
```

**Cytomic-Vorteil:** numerisch stabiler bei hoher Resonanz (Q ≈ 50+) und hohen
Frequenzen nahe Nyquist — Standard-Biquad instabil bei Q > 10 & fs nahe Nyquist.

## 4. Häufige Fehler

```rust
// FALSCH: Koeffizienten nicht neu berechnen bei SR-Wechsel
fn prepare(&mut self, sample_rate: f64, _: usize) {
    self.sample_rate = sample_rate as f32;
    // Filter-Struct nicht neu erstellt oder set_params() nicht aufgerufen!
}

// RICHTIG:
fn prepare(&mut self, sample_rate: f64, _: usize) {
    // Neu erstellen — günstig, garantiert konsistenten State:
    self.filter = BiquadFilter::new(
        FilterType::LowPass, sample_rate as f32, self.cutoff, self.q
    ).unwrap_or_else(|_| /* fallback */ ...);
    // Oder: set_params aufrufen wenn SR sich ändert
}
```

```rust
// FALSCH: q = 0.0 → DspError, aber unwrap() → Panic im Host
let f = BiquadFilter::new(FilterType::LowPass, 44100.0, 1000.0, 0.0).unwrap();

// RICHTIG: q aus User-Param clampen vor Übergabe
let q = param_q.value().max(0.01);  // nie 0
let f = BiquadFilter::new(FilterType::LowPass, sr, cutoff, q)?;
```

## 5. Kaskadierung (Steilere Flanken)

```rust
// 2× BiquadFilter in Serie = 24 dB/oct (4th-order Butterworth-ähnlich)
// Q-Werte für maximally-flat Butterworth-Kaskade:
// Stage 1: Q = 0.5412, Stage 2: Q = 1.3066
struct SteepLowPass {
    stage1: BiquadFilter,
    stage2: BiquadFilter,
}

fn process_sample(&mut self, input: f32) -> f32 {
    self.stage2.process_sample(self.stage1.process_sample(input))
}
```

## 6. Weitere Filter in aura-dsp

```rust
// Moog Ladder (4-Pol, resonant, klassisch):
use aura_dsp::filter::MoogLadder;

// Predictive Ladder (ZDF linear prediction, InfiniteDSP):
use aura_dsp::filter::PredictiveLadder;
```

## See also

- [filter-biquad.md](./filter-biquad.md) — Stabilität, Pole-Radius-Check
- [dsp-denormals.md](./dsp-denormals.md) — BiquadFilter/SVF intern schon gesichert
- [dsp-correctness.md](./dsp-correctness.md) — SR-Wechsel, reset()
