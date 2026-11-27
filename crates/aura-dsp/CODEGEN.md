# DSP block/codegen evidence

Measured 2026-11-27 on Windows x86_64, AMD Ryzen 7 5800H, rustc 1.99.1.
No explicit SIMD, fast-math, voice batching or new external dependency.
Only the final benchmark/example target uses `-C target-cpu=native`; dependencies
use workspace flags. Native binaries are for this measurement machine, not a
portable deployment recommendation.

## Reproduce

```powershell
cargo rustc -p lx-aura-dsp --example block_codegen --release -- --emit=asm -C target-cpu=native
python crates/aura-dsp/tools/check_codegen.py target/release/examples/block_codegen.s
cargo rustc -p lx-aura-dsp --bench benchmarks --release -- -C target-cpu=native
$benchFile = Get-ChildItem target/release/deps/benchmarks-*.exe | Sort-Object LastWriteTime -Descending | Select-Object -First 1
& $benchFile.FullName --bench block_first --sample-size 40 --warm-up-time 1 --measurement-time 3 --save-baseline block-first-final
```

The assembly checker deliberately rejects the unchanged legacy FIR as a negative
control: double audio arithmetic and `& 63` history indexing. It accepts the new
IIR/mix/DC paths without double arithmetic, gathers or FIR history masks. Delay
modulo occurs between linear interpolation spans; the checker verifies absence
of integer division/modulo in the basic blocks containing vector interpolation.
Biquad preparation may use f64; its audio recurrence uses scalar f32 FMA.
This check targets rustc's Windows x86_64 AT&T assembly on an FMA-capable CPU.

Actual emitted instructions, abbreviated from `probe_*` symbols:

```asm
; IIR: LLVM packs independent allpass branches; recurrence over time remains.
vfmadd231ps %xmm15, %xmm4, %xmm0
vfmadd231ps %xmm5, %xmm0, %xmm4
; Mix: independent contiguous audio samples.
vfmadd213ps (%rcx,%r10), %ymm1, %ymm2
vfmadd213ps 32(%rcx,%r10), %ymm1, %ymm3
; Biquad / DC: deliberately scalar over time.
vfmadd213ss %xmm8, %xmm0, %xmm7
vfmadd213ss %xmm3, %xmm0, %xmm1
; Delay tap interpolation: contiguous spans, no wrapped gather loop.
vmulps (%rsi), %ymm2, %ymm4
vmulps -32(%rsi), %ymm2, %ymm5
```

## Criterion

256 host samples, finite sine input, persistent state, FTZ/DAZ set once outside
measurement loops. Converter callbacks use the same hard clip at +/-0.4. Block
paths include source-buffer copy and boundary validation. Constructors and
allocation occur outside timing. Final run performed after workspace checks,
without a concurrent build. Table shows Criterion estimates and 95% intervals.

| Path | Time per 256 samples | 95% interval |
| --- | ---: | ---: |
| Legacy 127-tap FIR sample closure | 23.653 us | 23.239..23.999 us |
| Legacy IIR sample closure | 8.6109 us | 8.0945..9.0613 us |
| New IIR block callback | 4.5722 us | 4.4670..4.6634 us |
| Previous biquad f64 TDF-II tick | 1.4229 us | 1.4161..1.4294 us |
| New biquad one-sample wrapper | 1.7249 us | 1.6031..1.8658 us |
| New biquad block | 0.83529 us | 0.82506..0.84641 us |

In this run: IIR block ~5.2x faster than the legacy FIR and ~1.9x faster than
legacy IIR sample callbacks. Biquad block ~1.7x faster than old f64 processing.
The new biquad sample wrapper is ~21% slower than old sample processing: use the
block API to amortize boundary validation/coefficient-cache checks. These are
local converter/biquad measurements, not Helmira/MOSAIC voice or patch results.
Different targets/block sizes/nonlinear callbacks need their own measurements.
`mul_add` can lower differently on CPUs without hardware FMA.

## Regression and RT evidence

- New block APIs initially fail to compile against the baseline. Biquad invalid
  block test initially returns `[0.003916077, 0, 0.018857155]` instead of silence.
- Review-driven failing tests catch a ~9516x state-rescaling spike and rejection
  of valid cancelled-pole identity coefficients. Fixed by coefficient-independent
  TDF-II history and an endpoint-aware, division-free delta realization.
- 192 rate/cutoff/Q/filter combinations compared with independent f64 TDF-II.
  Long 1 Hz and near-Nyquist tests, sign-mode transitions, serialization, block
  partitions and existing sample-rate-retarget tests pass.
- Linear IIR scratch tested across internal chunk boundaries against the legacy
  scalar reference. DC, -60 dB imaging/decimation rejection, documented 3-sample
  impulse peak, finite recovery and stateful callback ordering pass.
- Counting allocator observes zero alloc/dealloc events across converter,
  biquad, DC, mix, delay-tap processing and reset/guard operations.
- Actual CLAP callback test fails with FTZ/DAZ setup removed and passes restored;
  separate core test verifies host MXCSR restoration. Guard is thread-bound.
- Workspace build/tests, clippy all targets with `-D warnings`, format and
  whitespace checks pass. Existing ignored tests retain their normal status.

## Intentional boundaries

The legacy FIR and existing IIR sample-callback adapter retain their numerics.
The new IIR block method uses FMA and chunk-level invalid recovery. IIR group
delay varies with frequency; `LATENCY_SAMPLES = 3` denotes the identity impulse
peak, not a phase-aligned dry/wet delay.

Moog/Steiner nonlinear recurrences remain scalar on purpose. Existing Moog ADAA
uses f64 primitive differences for accuracy; Steiner retains f64 recursive state
for low-cutoff DC accuracy. Migrating those would require separate numerical
work; this PR makes no SIMD-over-time or all-legacy-kernels-migrated claim.
Non-x86_64 products/hosts must configure their platform denormal mode themselves.
Helmira and its mixing law are untouched.
