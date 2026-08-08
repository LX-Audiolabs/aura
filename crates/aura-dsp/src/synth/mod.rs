//! Synthesis algorithm modules.
//!
//! Each module provides a composable synthesis algorithm that
//! dhvani wires into playable instruments. These are not
//! instruments themselves — they are the DSP building blocks
//! that produce and shape sound.

pub mod additive;
pub mod drum;
pub mod fm;
pub mod formant;
pub mod granular;
pub mod physical;
pub mod subtractive;
pub mod vocoder;
