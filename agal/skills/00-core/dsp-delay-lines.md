---
id: dsp-delay-lines
group: core
summary: DelayLine, CombFilter, AllpassDelay aus aura-dsp — fraktionale Delays, Interpolation.
triggers: delay, delay line, ring buffer, fractional delay, interpolation, comb filter, allpass, reverb, chorus, dsp
verify: max_delay in prepare() allokiert; flush_denormal in Feedback-Pfaden; delay_samples <= max-1
source: global
copied_by: template
date: 2026-08-25
adapted: true
reason: "aura_dsp::delay hat kanonische Implementierungen — nicht neu implementieren"
---

# Delay Lines, Comb Filter, Allpass Delay

**Summary:** `aura_dsp::delay` liefert `DelayLine` (Ringbuffer + fraktionale Interpolation),
`CombFilter` (Feedback-Comb) und `AllpassDelay` (Schroeder single-buffer).
Alle in `prepare()` mit maximaler Delay-Zeit allokieren — nicht im Audio-Thread.

## 1. `DelayLine` — Ringbuffer mit fraktionalem Delay

```rust
use aura_dsp::delay::DelayLine;

// In prepare(): max_delay_samples = max_delay_sek * sample_rate
let max_samples = (max_delay_sek * sample_rate).ceil() as usize;
let mut delay = DelayLine::new(max_samples);

// Im process():
delay.write(input_sample);
let delayed = delay.read(delay_samples);  // delay_samples kann f32 sein (fraktional)
```

**Interne Implementierung:** Linearer Interpolation zwischen benachbarten Samples.
- `read(0.0)` → zuletzt geschriebenes Sample
- `read(n)` → n Samples zurück
- Clampt automatisch auf `[0, max-1]`

**Warnung:** `delay_samples` muss < `max_delay_samples` sein — kein Panic, aber
Clamping gibt falschen Wert bei Überschreitung.

## 2. Interpolation: Linear vs. Hermite

```rust
// Linear (in DelayLine::read() intern) — OK für Reverb, Chorus:
let linear = delay.read(frac_delay);

// Hermite (bessere Qualität für Pitch-Shifting, Vibrato):
use aura_dsp::dsp_util::hermite_interpolate;

let d = frac_delay.floor() as usize;
let frac = frac_delay - d as f32;
let y0 = delay.read((d + 1) as f32);  // d-1 Sample (read mit +1 wegen Ringbuffer-Logik)
let y1 = delay.read(d as f32);
let y2 = delay.read((d - 1) as f32);  // d+1 Sample zurück
let y3 = delay.read((d - 2) as f32);
let hermite = hermite_interpolate(y0, y1, y2, y3, frac);
```

**Faustregel:**
- Reverb, Chorus, Echo: Linear reicht
- Pitch-Shifting, Vibrato, Granular: Hermite oder Allpass-Interpolation

## 3. `CombFilter` — Feedback Comb

```rust
use aura_dsp::delay::CombFilter;

// Stabilitätsbedingung: |feedback| < 1.0 — CombFilter clampt auf (-0.999, 0.999)
let mut comb = CombFilter::new(
    delay_samples,  // usize, integer delay
    0.7,            // feedback (−1..1)
);

// y[n] = x[n] + feedback * y[n - delay]
let output = comb.process_sample(input);
// Intern: flush_denormal auf delayed value — denormal-sicher
```

**Häufiger Fehler:** `feedback = 1.0` → endlose Verstärkung → Overflow.
`feedback = -1.0` → Phase-Inversion + endlos. Clampen auf `±0.999` reicht für Praxis.

## 4. `AllpassDelay` — Schroeder Single-Buffer

```rust
use aura_dsp::delay::AllpassDelay;

// y[n] = -g * x[n] + x[n-D] + g * y[n-D]
// Single-buffer-Design: halb so viel Speicher wie dual-buffer
let mut ap = AllpassDelay::new(
    delay_samples,  // usize
    0.5,            // coefficient (−1..1), clampt auf (-0.999, 0.999)
);

let output = ap.process_sample(input);
```

Typisch in Reverb-Netzwerken (Schroeder: 4 parallele Combs + 2 serielle Allpass).

## 5. Modulated Delay (Chorus/Vibrato/Flanger)

```rust
// LFO moduliert delay_samples; Smoother verhindert Zipper-Noise:
use aura_dsp::smoothing::ParamSmoother;

let mut delay_smoother = ParamSmoother::new(0.005, sample_rate, base_delay_samples);

fn process_sample(&mut self, input: f32) -> f32 {
    let lfo = self.lfo.next_sample();  // aus aura_dsp::modulation
    let target = self.base_delay + self.depth * lfo;
    self.delay_smoother.set_target(target);
    let current_delay = self.delay_smoother.next_value();

    self.delay.write(input);
    let delayed = self.delay.read(current_delay);
    input * self.dry + delayed * self.wet  // kein equal-power nötig bei kleinem depth
}
```

## 6. prepare() / reset()

```rust
fn prepare(&mut self, sample_rate: f64, max_block_size: usize) {
    let max_delay = (MAX_DELAY_SECS * sample_rate as f32).ceil() as usize;
    self.delay = DelayLine::new(max_delay);  // alloziert hier, nicht in process()
    self.smoother = ParamSmoother::new(0.005, sample_rate as f32, self.base_delay);
}

fn reset(&mut self) {
    self.delay.clear();  // State löschen ohne neu allozieren
    self.smoother.snap();
}
```

## See also

- [dsp-denormals.md](./dsp-denormals.md) — CombFilter intern schon gesichert
- [dsp-smoothing.md](./dsp-smoothing.md) — modulated delay braucht Smoother
- [dsp-realtime.md](./dsp-realtime.md) — kein Alloc in process()
