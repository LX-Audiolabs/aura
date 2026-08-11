//! # aura-dsp — Audio DSP primitives
//!
//! JUCE analogue: **`juce_dsp`** (processors, filters, oscillators, FX, maths).
//! MIDI messages/buffers live in **`aura-midi`** (not here).
//!
//! Core synthesis/FX stack forked from **naad** (Sanskrit: primordial sound)
//! and now maintained as part of AURA:
//! oscillators, filters, envelopes, wavetables, modulation, delay, effects,
//! noise, tuning. Product-grade LX FX / analysis modules land under this crate
//! over time (`fx/`, `analysis/`) — not as separate product-only silos in the
//! framework tree.
//!
//! Historical: this crate was forked from naad v1.2.5 (GPL-3.0-only,
//! part of the AGNOS ecosystem — dhvani / svara).
//!
//! ## Architecture
//!
//! All processing methods work on contiguous `&mut [f32]` slices. Hot-path
//! methods (`next_sample`, `process_sample`) are `#[inline]` for cross-crate
//! optimization. Buffer methods (`fill_buffer`, `process_buffer`) iterate
//! over slices, enabling SIMD auto-vectorization when compiled with
//! appropriate target features (`-C target-cpu=native`).
//!
//! dhvani (the sound engine) handles buffer alignment and SIMD dispatch.
//! aura-dsp provides the scalar reference implementations.
//!
//! ## Quick Start
//!
//! ```rust
//! use aura_dsp::oscillator::{Oscillator, Waveform};
//! use aura_dsp::envelope::Adsr;
//! use aura_dsp::filter::{BiquadFilter, FilterType};
//!
//! // Create a 440 Hz sine oscillator
//! let mut osc = Oscillator::new(Waveform::Sine, 440.0, 44100.0).unwrap();
//!
//! // Create an ADSR envelope
//! let mut env = Adsr::new(0.01, 0.1, 0.7, 0.3).unwrap();
//!
//! // Create a low-pass filter at 2kHz
//! let mut filter = BiquadFilter::new(FilterType::LowPass, 44100.0, 2000.0, 0.707).unwrap();
//!
//! // Generate a sample
//! env.gate_on();
//! let sample = osc.next_sample() * env.next_value();
//! let filtered = filter.process_sample(sample);
//! ```
//!
//! ## Feature Flags
//!
//! - `default` — All core primitives (oscillators, filters, envelopes, effects, dynamics, EQ, reverb, voice, mod matrix, panning, smoothing, tuning, noise, delay, wavetable, dsp_util)
//! - `synthesis` — Synthesis algorithm modules (subtractive, FM, drum, formant, additive, vocoder, granular, physical modeling)
//! - `acoustics` — Room simulation, convolution reverb, binaural, FDN, analysis, ambisonics (via goonj)
//! - `logging` — Enable tracing-subscriber for structured logging output
//! - `full` — All features enabled
//!
//! ## API Conventions
//!
//! These conventions apply uniformly across the crate. Knowing them up front
//! avoids surprises when wiring aura-dsp into a host (dhvani, svara, etc.).
//!
//! ### Encapsulation
//!
//! - **Stateful types with non-trivial invariants** (e.g. [`oscillator::Oscillator`],
//!   [`filter::BiquadFilter`], [`wavetable::Wavetable`]) keep their fields **private**
//!   and expose validated setters. Modifying state requires a constructor or a
//!   typed mutator — there is no way to silently break the invariant.
//! - **Parameter structs** that are pure value-bags read directly by their owning
//!   type (e.g. [`dynamics::Compressor::threshold_db`], [`envelope::Adsr::sustain_level`])
//!   keep `pub` fields. They are documented when modifying them bypasses
//!   constructor-time clamping; callers may set arbitrary `f32` values and
//!   accept whatever output the algorithm produces.
//!
//! Rule of thumb: if changing a field requires recomputing cached coefficients
//! or affects internal state coupling, it is private. If it only feeds a
//! direct read in the next-sample loop, it may be `pub`.
//!
//! ### Constructor return types
//!
//! - **Constructors that validate `sample_rate`, `frequency`, or other
//!   range-restricted inputs return [`Result`]**. Examples:
//!   [`oscillator::Oscillator::new`], [`filter::BiquadFilter::new`],
//!   [`envelope::Adsr::new`].
//! - **Constructors that only clamp inputs (e.g. amplitude to `0..=1`) are
//!   infallible** — they take any `f32` and store the clamped result.
//!   Examples: [`dynamics::LevelDetector::new`], [`smoothing::ParamSmoother::new`].
//! - **Index-based mutators (`set_operator_freq`, `set_band_gain`)** that may
//!   receive an out-of-range index return `Option<()>` or `Result<()>` so
//!   callers can detect — and not silently ignore — bad indices.

/// Portable analysis: FFT/SNAP maths, spectrum, meter blocks (no vault/MD/*Shared).
pub mod analysis;
pub mod delay;
pub mod dsp_util;
pub mod dynamics;
pub mod effects;
pub mod envelope;
pub mod eq;
pub mod error;
pub mod filter;
/// LX product-grade FX (from `lx-dsp`): TDF-II biquad, mastering, meters, …
pub mod fx;
pub mod mod_matrix;
pub mod modulation;
pub mod noise;
pub mod oscillator;
pub mod panning;
pub mod reverb;
pub mod smoothing;
pub mod tuning;
pub mod voice;
pub mod wavetable;

#[cfg(feature = "synthesis")]
pub mod synth;

#[cfg(feature = "acoustics")]
pub mod acoustics;

pub use error::{DspError, Result};

/// Flush denormal floating-point values to zero.
///
/// Denormal (subnormal) floats cause 10-100x slowdowns on x86 processors.
/// Call this on filter state variables and feedback paths to prevent
/// performance degradation when signals decay to near-zero.
#[inline]
#[must_use]
pub fn flush_denormal(x: f32) -> f32 {
    // A float is denormal if its exponent bits are all zero but mantissa is non-zero.
    // Threshold: f32::MIN_POSITIVE (1.175e-38) is the smallest normal f32.
    if x.abs() < f32::MIN_POSITIVE { 0.0 } else { x }
}
