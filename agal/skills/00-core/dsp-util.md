---
id: dsp-util
group: core
summary: dsp_util aus aura-dsp — dB, Interpolation, LUT, PRNG, Polynomial-Approximation.
triggers: db, amplitude, lerp, hermite, interpolation, LUT, lookup table, PRNG, noise, polynomial, tanh, powf, dsp
verify: db_to_amplitude_lut statt powf in Hot Path; xorshift32 statt rand in Audio-Thread
source: global
copied_by: template
date: 2026-08-25
adapted: true
reason: "aura_dsp::dsp_util hat fertige Lösungen — nicht neu implementieren"
---

# DSP Utilities (aura_dsp::dsp_util)

**Summary:** `aura_dsp::dsp_util` sammelt shared Free Functions die überall gebraucht
werden: dB↔Amplitude, Interpolation, LUT, PRNG, Fenster-Funktionen.
Alles `#[inline]` — kein Overhead gegenüber direktem Code.

## 1. dB ↔ Amplitude

```rust
use aura_dsp::dsp_util::{amplitude_to_db, db_to_amplitude, db_to_amplitude_lut};

// Exakt (per `powf` / `log10`):
let db = amplitude_to_db(0.5);   // → -6.02 dB
let amp = db_to_amplitude(-6.0); // → 0.501

// SCHNELL: LUT-Version für Hot Paths (256 Einträge, ±0.5% Fehler):
// Bereich: [-80, +20] dB — außerhalb: clampt auf Tabellenränder
let amp_fast = db_to_amplitude_lut(gain_db); // Avoidiert powf im Sample-Loop

// Compressor/Limiter Gain-Stage: db_to_amplitude_lut() statt powf()
let gain = db_to_amplitude_lut(computed_gain_db);
*sample *= gain;
```

**Wann LUT vs. exakt?**
- Audio-Thread innerhalb des Sample-Loops → LUT
- Koeffizienten-Berechnung in `prepare()` / `set_params()` → exakt

## 2. Interpolation

```rust
use aura_dsp::dsp_util::{lerp, hermite_interpolate};

// Lineare Interpolation:
let val = lerp(a, b, 0.5);  // Midpoint

// Cubic Hermite (4-Punkt, C¹-stetig, ideal für Wavetable/Delay-Interpolation):
// y0, y1, y2, y3 = 4 aufeinanderfolgende Samples, t ∈ [0, 1] zwischen y1 und y2
let val = hermite_interpolate(y0, y1, y2, y3, frac);
// Genauer als linear, billiger als Sinc — Standard für Pitch-Shifting
```

## 3. PRNG — Noise ohne `rand`-Crate

```rust
use aura_dsp::dsp_util::{xorshift32, xorshift32_signed_f32, xorshift32_unit_f32};

// State als u32 im Struct — thread-safe wenn Audio-Thread der einzige Schreiber:
struct MyPlugin {
    prng_state: u32,
}

// In prepare(): Seed setzen
self.prng_state = 12345u32;  // oder: std::time::SystemTime hash

// Im process():
let noise_sample = xorshift32_signed_f32(&mut self.prng_state);  // [-1.0, 1.0)
let noise_unit = xorshift32_unit_f32(&mut self.prng_state);       // [0.0, 1.0)

// Raw u32 (für Bit-Manipulation / Granular):
let raw = xorshift32(&mut self.prng_state);
```

**Wichtig:** Xorshift32 hat eine Zero-State-Guard (`state == 0 → state = 1` intern).
Kein extra Guard nötig.

## 4. Polynomial-Approximation für teure Funktionen

```rust
use aura_dsp::dsp_util::{fit_polynomial, eval_polynomial};

// Einmalig: tanh durch Polynom ersetzen (billiger als tanh() im Loop)
// Requires: "synthesis" Feature
#[cfg(feature = "synthesis")]
fn build_tanh_approx() -> Vec<f64> {
    let xs: Vec<f64> = (0..201).map(|i| -2.0 + i as f64 * 0.02).collect();
    let ys: Vec<f64> = xs.iter().map(|&x| x.tanh()).collect();
    fit_polynomial(&xs, &ys, 5).expect("tanh fit")  // Grad-5 reicht für ±1.5
}

// In prepare(): coeffs berechnen und cachen
self.tanh_coeffs = build_tanh_approx();

// Im process() (Horner-Methode, 6 multiply-add statt 1 exp):
let sat = eval_polynomial(&self.tanh_coeffs, input * drive);
// Accuracy: < 1.5% Fehler in [-1.5, 1.5] — ausreichend für Soft-Clip
```

## 5. Soft-Clip

```rust
use aura_dsp::dsp_util::soft_clip_tanh;

// drive > 1.0 = mehr Sättigung; 1.0 = mild
let saturated = soft_clip_tanh(input, drive);
// Intern: (input * drive).tanh()
// Für Hot Path: stattdessen polynomial approximation verwenden (Punkt 4)
```

## 6. Equal-Power Crossfade (Wet/Dry)

```rust
use aura_dsp::dsp_util::crossfade_equal_power;

// mix = 0.0 → 100% dry, 1.0 → 100% wet
// Nutzt cos/sin statt linearen Fade → kein "Loch" bei 50%
let output = crossfade_equal_power(dry, wet, mix);
```

## 7. Analyse-Utilities (Metering)

```rust
use aura_dsp::dsp_util::{rms, peak, normalize};

// RMS eines Blocks:
let level_rms = rms(&buffer);  // sqrt(sum(x²)/n)

// Peak:
let level_peak = peak(&buffer);  // max(|x|)

// Normalisieren (nicht im Audio-Thread!):
normalize(&mut buffer);  // teilt durch peak → Alloc-frei, aber nur für Analyse
```

## See also

- [dsp-delay-lines.md](./dsp-delay-lines.md) — hermite_interpolate für Delay
- [dsp-smoothing.md](./dsp-smoothing.md) — ParamSmoother statt manuelles lerp
- [dsp-realtime.md](./dsp-realtime.md) — kein powf / exp im Sample-Loop
