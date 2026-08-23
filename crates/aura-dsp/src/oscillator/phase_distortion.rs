//! Phase-distortion / vector-synthesis oscillator skeleton.
//!
//! This module will house a phase-distortion oscillator with two-dimensional
//! shape control (`shape_x`, `shape_y`) suitable for vector-synthesis morphing.
//! For now it exposes only the type scaffolding so downstream modules can
//! depend on the API surface.

#[derive(Debug, Clone)]
pub struct PhaseDistortionOscillator {
    phase: f32,
    sample_rate: f32,
    shape_x: f32,
    shape_y: f32,
}
