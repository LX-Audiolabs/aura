# aura-dsp

Portable DSP for [AURA](https://github.com/LX-Audiolabs/aura): oscillators,
filters, FX, analysis. Optional via `aura` feature `dsp` (default on).

Third-party provenance: see repository root [`NOTICE`](../../NOTICE).

## Oversampling

`aura_dsp::oversampling::Oversampler2x` wraps caller DSP with halfband
interpolation and filtered decimation. Construct one instance per channel before
rendering; `process_sample(input, |x| ...)` invokes the closure twice, in order.
Configure the complete enclosed DSP chain at twice the host sample rate. Nest
two instances for 4x and configure the innermost DSP at four times the host rate.
See [module examples](src/oversampling.rs) for Steiner and tanh wiring.

The fixed 127-tap Blackman halfband FIR uses polyphase processing, fixed arrays
and double-precision accumulation. Processing/reset allocate nothing and take no
locks; the caller's closure must obey the same rules. Nonfinite samples are
replaced with zero; subnormal history/output values are flushed. No dependency
or changes to Steiner/Moog APIs are required. Helmira integration is separate.

Conversion latency is 63 host samples at 2x, 94.5 at 4x, excluding caller DSP.
Account for the fractional delay when aligning dry/wet paths or reporting host
latency. At 48 kHz the 2x pair retains DC..19.2 kHz within 0.02 dB; the halfband
transition extends to 28.8 kHz. Host Nyquist is attenuated by approximately
6 dB. This is a bandwidth/latency tradeoff, not transparent full-band bypass.

### Alias measurement

Run `cargo test -p lx-aura-dsp --test oversampling -- --nocapture`.
The test drives a unit-peak sine through `tanh(drive * x)` at a 48 kHz host rate.
It discards 8192 startup samples, then analyses 8192 coherent samples without a
window. Values below are the largest nonharmonic baseband FFT bin relative to
the fundamental (dBc), not integrated alias power or a perceptual rating.
The independent 4x reference directly samples the analytic sine at 192 kHz,
applies f64 tanh and retains baseband FFT bins (ideal low-pass decimation).

| Sine (Hz) | Drive | 1x | 2x | 4x cascade | Independent 4x reference |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 5841.796875 | 2 | -28.30 | -78.47 | -115.47 | -178.70 |
| 5841.796875 | 4 | -18.83 | -45.75 | -99.17 | -99.01 |
| 9978.515625 | 2 | -15.45 | -53.41 | -103.58 | -103.53 |
| 9978.515625 | 4 | -11.31 | -32.41 | -59.12 | -59.07 |

For these stimuli, 2x improves the peak alias by 21..50 dB. Strong drive near
10 kHz still leaves -32.41 dBc at 2x; 4x provides another 26.71 dB reduction.
Even the 4x reference has nonlinear aliases, so it is not an alias-free truth.
These measurements justify offering 2x and the 4x composition; they do not select
a universal factor for Helmira or characterize its stateful Moog/Steiner paths.
Those paths and their modulation need separate measurements when integrated.

License: GPL-3.0-or-later.
