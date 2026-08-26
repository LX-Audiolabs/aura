---
id: dsp-smoothing
group: core
summary: Parameter-Smoothing gegen Zipper-Noise — ParamSmoother aus aura-dsp/aura-params.
triggers: smoothing, zipper, click, parameter change, knob, automation, ParamSmoother, dsp
verify: kein direktes Schreiben von Param-Wert in Audio-Loop; is_settled() für CPU-Idle
source: global
copied_by: template
date: 2026-08-25
adapted: true
reason: "aura-dsp/aura-params haben kanonische Smoother — nicht neu implementieren"
---

# Parameter Smoothing (Zipper-Noise-Prävention)

**Summary:** Direkte Param-Änderungen im Audio-Thread erzeugen Zipper-Noise (hörbare
Stufen-Artefakte). `aura_dsp::smoothing::ParamSmoother` ist der kanonische
One-Pole-EMA-Smoother in AURA — verwenden, nicht nachbauen.

## 1. Kanonischer Typ: `ParamSmoother`

```rust
use aura_dsp::smoothing::ParamSmoother;

// In prepare() / reset():
let mut smoother = ParamSmoother::new(
    0.005,       // smooth_time: Zeitkonstante in Sekunden (~63% in dieser Zeit)
    sample_rate, // SR aus prepare()
    initial_val, // Startwert — sofort, kein Einpendeln nötig
);

// Im process()-Loop (pro Sample oder pro Block):
smoother.set_target(new_value);  // Thread-safe über Atomic→GUI; hier nur lesen
let smoothed = smoother.next_value();  // EMA-Schritt: current += coeff * (target - current)
```

**EMA-Koeffizient:** `coeff = 1 - exp(-1 / (time * sr))`
→ Zeitkonstante = Zeit bis 63% des Zielwerts erreicht. Für 5ms bei 44.1kHz ≈ 0.00226.

## 2. Häufige Fehler

```rust
// FALSCH: Direkte Zuweisung im Audio-Thread → Zipper-Noise
self.gain = param.value();  // Stufensprung hörbar

// RICHTIG: Smoother als State im Plugin-Struct
struct MyPlugin {
    gain_smoother: ParamSmoother,
}

fn process(&mut self, ...) {
    self.gain_smoother.set_target(self.params.gain.value());
    let gain = self.gain_smoother.next_value();
    // ...
}
```

## 3. Nützliche Methoden

```rust
// Sofort auf Zielwert springen (nach reset() / Transport-Stop / SR-Wechsel):
smoother.snap();

// CPU-Optimierung: keine Smoothing-Arbeit wenn bereits am Ziel:
if smoother.is_settled() {
    // Alle Samples im Block mit smoother.current() multiplizieren (konstant)
    buffer.iter_mut().for_each(|s| *s *= smoother.current());
} else {
    // Per-Sample smoothing nötig
    for s in buffer.iter_mut() {
        *s *= smoother.next_value();
    }
}

// Smooth-Zeit nachjustieren (z.B. nach SR-Wechsel in prepare()):
smoother.set_smooth_time(new_time);
```

## 4. Smooth-Zeit Richtwerte

| Anwendung | smooth_time |
|-----------|------------|
| Gain/Volume (Klick-frei) | 5–20 ms |
| Filter Cutoff | 10–30 ms |
| Pan | 5 ms |
| Pitch-Shift | 50–100 ms |
| Bypass-Crossfade | 20–50 ms |

## 5. Mehrere Parameter

```rust
// Ein Smoother pro Parameter — nicht teilen!
struct MyPlugin {
    gain_smoother: ParamSmoother,
    cutoff_smoother: ParamSmoother,
    resonance_smoother: ParamSmoother,
}

// In prepare(): alle mit sample_rate und Startwert initialisieren
// In reset(): alle .snap() aufrufen → kein Einpendeln nach Pause
```

## 6. aura-params `smooth.rs`

`aura-params` hat ebenfalls Smoother (wraps `ParamSmoother` mit CLAP-Param-Integration).
Beim direkten aura-dsp-Einsatz: `aura_dsp::smoothing::ParamSmoother` direkt.

## Verifizieren

- Param auf Maximalwert setzen, sofort auf Minimum → kein hörbarer Klick
- `is_settled()` in Hot Path: sicherstellen dass es nicht dauerhaft `false` bleibt
  (Epsilon = `1e-6` — ausreichend für Audio)

## See also

- [dsp-realtime.md](./dsp-realtime.md) — Parameter einmal pro Block samplen
- [dsp-denormals.md](./dsp-denormals.md) — flush_denormal in next_value() bereits drin
- [dsp-correctness.md](./dsp-correctness.md) — Klickfreiheit nach Reset/SR-Wechsel
