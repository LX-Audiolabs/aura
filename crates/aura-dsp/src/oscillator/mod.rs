//! Oscillator module with band-limited waveform generation.
//!
//! Provides PolyBLEP anti-aliased saw, square, and pulse waveforms,
//! along with basic sine, triangle, and noise generators. Layered
//! variants (unison, sub-oscillator, hard sync) build on the base
//! [`Oscillator`].
//!
//! Submodule layout (1.1.0):
//! - [`core`] — [`Waveform`], [`Oscillator`], [`polyblep`]
//! - [`unison`] — [`UnisonOscillator`] (1–8 voice detune + stereo spread)
//! - [`sub`] — [`SubOscillator`], [`SubOctave`] (octave-divided layer)
//! - [`sync`] — [`HardSync`] (master/slave phase-reset pair)
//!
//! All public types are re-exported at the module root, so external
//! callers continue to use `naad::oscillator::Oscillator` etc.

pub mod core;
pub mod sub;
pub mod sync;
pub mod unison;

pub use core::{Oscillator, Waveform, polyblep};
pub use sub::{SubOctave, SubOscillator};
pub use sync::HardSync;
pub use unison::UnisonOscillator;
