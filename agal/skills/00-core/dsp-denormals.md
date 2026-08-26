---
id: dsp-denormals
group: core
summary: Subnormal float schutz — flush_denormal(), FTZ/DAZ, Feedback-Pfade in AURA.
triggers: denormal, subnormal, performance, slowdown, flush, FTZ, DAZ, feedback, IIR, dsp
verify: flush_denormal() an jeder State-Variable in Feedback-Pfaden; CombFilter/BiquadFilter schon intern gesichert
source: global
copied_by: template
date: 2026-08-25
adapted: true
reason: "aura-dsp hat flush_denormal() als kanonische Implementierung — darauf bauen"
---

# Denormal / Subnormal Float Schutz

**Summary:** Subnormale Floats (Exponent = 0, Mantisse ≠ 0) erzeugen 10–100× CPU-Overhead
auf x86 — typisch wenn IIR-State oder Feedback-Signale gegen Null ausschwingen.
`aura_dsp::flush_denormal()` ist die kanonische Lösung; alle Hot-Path-Filter in
aura-dsp rufen sie bereits intern auf.

## 1. Kanonische Funktion: `aura_dsp::flush_denormal()`

```rust
// Aus aura_dsp::lib.rs — bereits im Crate verfügbar:
#[inline]
pub fn flush_denormal(x: f32) -> f32 {
    if x.abs() < f32::MIN_POSITIVE { 0.0 } else { x }
}
```

- Threshold: `f32::MIN_POSITIVE` (1.175e-38) — kleinste normale f32
- Branchless-freundlich durch Compiler-Optimierung
- **Nicht** `1e-20` als Magic Number — `f32::MIN_POSITIVE` ist der korrekte Wert

## 2. Was ist schon geschützt (AURA intern)

Diese Typen rufen `flush_denormal()` bereits in ihren Hot Paths:

| Typ | Pfad | geschützt in |
|-----|------|-------------|
| `BiquadFilter` | `filter/mod.rs` | `process_sample()` — z1, z2 |
| `StateVariableFilter` | `filter/mod.rs` | `process_sample()` — ic1eq, ic2eq |
| `CombFilter` | `delay.rs` | `process_sample()` — feedback delayed |
| `ParamSmoother` | `smoothing.rs` | `next_value()` — current |

**Nicht** automatisch geschützt: eigene Feedback-Schleifen, Reverb-Netzwerke,
physikalische Modellierung, custom Delay-Netzwerke.

## 3. Wo `flush_denormal()` manuell aufrufen

```rust
// Eigener Feedback-Pfad (z.B. Karplus-Strong, Schroeder-Reverb):
let feedback = self.delay.read(self.delay_samples);
let out = input + self.coeff * aura_dsp::flush_denormal(feedback);
self.delay.write(out);

// State-Variables in eigenem IIR:
self.z1 = aura_dsp::flush_denormal(b1 * input - a1 * output + self.z2);
self.z2 = aura_dsp::flush_denormal(b2 * input - a2 * output);

// Nach langen Silence-Phasen im State-Reset:
pub fn reset(&mut self) {
    self.z1 = 0.0;  // 0.0 ist nie subnormal — kein flush nötig
    self.z2 = 0.0;
}
```

## 4. Wann ist es ein Problem?

- IIR-Filter mit hohem Q / sehr tiefer Cutoff → State klingt langsam aus
- Reverb-Netzwerke (FDN, Schroeder) → viele Feedback-Pfade
- Karplus-Strong / physikalische Modelle → sehr lange Ausschwingzeit
- Envelope-Follower / Compressor-Detector → Signal kann gegen Null fallen

## 5. FTZ/DAZ als Alternative (nicht bevorzugt)

```rust
// x86-only, nicht portable, wirkt global auf Thread-Ebene:
#[cfg(target_arch = "x86_64")]
unsafe {
    std::arch::x86_64::_MM_SET_FLUSH_ZERO_MODE(
        std::arch::x86_64::_MM_FLUSH_ZERO_ON
    );
}
```

AURA-Präferenz: **`flush_denormal()` pro State-Variable** statt globales FTZ-Flag,
weil FTZ auch legitime Subnormals in anderen Crates flusht.

## Verifizieren

- Profiler (perf/VTune): hohe Cycles bei stillem Signal → Denormal-Verdacht
- Debug-Build: `if x.abs() < f32::MIN_POSITIVE && x != 0.0 { log_once!("denormal") }`
- Test: Filter mit Impulse anregen, dann 10k stille Samples — kein CPU-Spike

## See also

- [filter-biquad.md](./filter-biquad.md) — TDF-II intern schon gesichert
- [dsp-delay-lines.md](./dsp-delay-lines.md) — CombFilter, AllpassDelay
- [dsp-realtime.md](./dsp-realtime.md) — §4 Numerik & Struktur
