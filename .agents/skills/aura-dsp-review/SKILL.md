---
name: aura-dsp-review
description: Review AURA DSP and process() code for realtime safety and numeric correctness, using AURA-specific rules (AudioBuffer, ProcessContext, NoteVoiceTable, FTZ/DAZ, smoothers). Trigger on "review my process()", "check this DSP", "is this realtime-safe", or any audio-thread code in the AURA codebase.
---

# AURA DSP Review (realtime + numerics)

Review `process()` and DSP helpers for two things: **realtime safety** and
**numeric correctness**, using AURA's actual API. This skill is AURA-specific;
for the generic checklists, load the global skills first if present —
`audio-dsp-review` (realtime) and `audio-numerics-review` (numerics). This file
only adds what AURA changes.

## AURA realtime contract

`PluginLogic::process` runs on the audio thread. The following are violations:

- Allocation (`Vec::push` with growth, `String`, `Box`, `format!`, `to_string`).
- Locks (`Mutex`, `RwLock`, `parking_lot`) — use atomics / `triple_buffer`.
- File I/O, `println!`/`eprintln!`, logging that isn't a lock-free ring buffer.
- Any `panic!`/`unwrap()` that can unwind across the C ABI (CLAP/VST3/LV2).

Pre-allocate in `init`/`reset`; `process` only mutates.

## AURA types to check

- `AudioBuffer<'_, f32>` — inputs/outputs as channel slices; loop over channels,
  never hardcode 2 (bus layouts can be mono/stereo/sidechain).
- `ProcessContext` — event buffers (`midi`, `notes`, `ump`, `*_out`) are already
  sorted by sample offset. Read them; don't reallocate.
- `NoteVoiceTable` — fixed-size pool (`new(max_voices)` clamps to 1..=128);
  `apply`/`flush_ends` are allocation-free after `new`. Prefer it over a
  hand-rolled `Vec` of voices.
- `ProcessStatus` — return `Continue` normally; `TailFinished`/`Sleep` only when
  you know the host semantics.

## FTZ / DAZ (denormals)

- `aura::FtzDazGuard` (`aura_core::process::FtzDazGuard`, re-exported as
  `aura_dsp::fx::FtzDazGuard`) sets FTZ/DAZ on `x86_64` and restores MXCSR on
  drop. The format wrappers enable it once per callback edge.
- Standalone DSP users (no format wrapper) must create the guard themselves at
  the callback edge — otherwise denormals can stall scalar IIR/filter kernels.
- On non-`x86_64` it's a no-op; the host/product owns the platform denormal mode.

## Numeric checks (AURA specifics)

- Block kernels are scalar-over-time; validate finite input and silence +
  reset state on non-finite / invalid (see `dc_blocker`, `delay`, `biquad`).
- Smoothers: params reach DSP through smoothers. Ensure `snap_smoothers()` after
  any direct restore; don't fight automation with hand-rolled ramps.
- Sample-accurate automation differs by format: CLAP applies `CHUNKED` params
  with block splits; VST3 applies the last point per block. Write DSP that
  tolerates both.

## Validation

- `cargo +nightly miri test -p <crate>` for `unsafe` (MXCSR, FFI).
- `cargo fuzz` targets for state-load / event parsing.
- `cargo clippy --workspace --all-targets -- -D warnings` — deny `mutex_atomic`,
  no `println!` in DSP.

Report findings as: **violation**, **risk**, or **ok** — each with the file:line
and the fix. Don't rewrite DSP unless you've traced the caller.
