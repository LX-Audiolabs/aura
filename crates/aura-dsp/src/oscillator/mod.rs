//! Oscillator module with band-limited waveform generation.
//!
//! Provides `PolyBLEP` anti-aliased saw, square, and pulse waveforms,
//! along with basic sine, triangle, and noise generators. Layered
//! variants (unison, sub-oscillator, hard sync) build on the base
//! [`Oscillator`]. Extra ports from fundsp (MIT/Apache): DSF, soft saw,
//! Karplus–Strong pluck.
//!
//! Submodule layout:
//! - [`core`] — [`Waveform`], [`Oscillator`], [`polyblep`]
//! - [`dsf`] — [`DsfSaw`], [`DsfSquare`] (Moorer DSF, fundsp port)
//! - [`phase_distortion`] — [`PhaseDistortionOscillator`] (phase-distortion / vector-synthesis)
//! - [`soft_saw`] — [`SoftSaw`] (`1/n²` wavetable, fundsp soft-saw)
//! - [`pluck`] — [`Pluck`] (Karplus–Strong, fundsp port)
//! - [`unison`] — [`UnisonOscillator`] (1–8 voice detune + stereo spread)
//! - [`sub`] — [`SubOscillator`], [`SubOctave`] (octave-divided layer)
//! - [`sync`] — [`HardSync`] (master/slave phase-reset pair)
//!
//! All public types are re-exported at the module root, so external
//! callers continue to use `aura_dsp::oscillator::Oscillator` etc.

pub mod core;
pub mod dsf;
pub mod phase_distortion;
pub mod pluck;
pub mod soft_saw;
pub mod sub;
pub mod sync;
pub mod unison;

pub use core::{Oscillator, Waveform, polyblep};
pub use dsf::{DsfSaw, DsfSquare};
pub use phase_distortion::{PhaseDistortionOscillator, wavefold};
pub use pluck::Pluck;
pub use soft_saw::SoftSaw;
pub use sub::{SubOctave, SubOscillator};
pub use sync::HardSync;
pub use unison::UnisonOscillator;
