//! Modulation matrix for flexible source-to-destination routing.
//!
//! Provides a general-purpose N-slot modulation routing system where
//! modulation sources (LFOs, envelopes, MIDI CCs) are mapped to
//! synthesis destinations (pitch, filter, amplitude, etc.) with
//! configurable depth.

use serde::{Deserialize, Serialize};

/// Number of modulation source slots.
pub const NUM_SOURCES: usize = 8;

/// Number of modulation destination slots.
pub const NUM_DESTINATIONS: usize = 8;

/// Modulation source identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ModSource {
    /// First LFO output.
    Lfo1,
    /// Second LFO output.
    Lfo2,
    /// Amplitude envelope output.
    AmpEnvelope,
    /// Filter envelope output.
    FilterEnvelope,
    /// Note velocity (0..1).
    Velocity,
    /// Mod wheel (CC1, 0..1).
    ModWheel,
    /// Channel aftertouch (0..1).
    Aftertouch,
    /// Pitch bend (-1..+1).
    PitchBend,
}

impl ModSource {
    /// Map source to array index (decoupled from enum discriminant).
    #[inline]
    fn index(self) -> usize {
        match self {
            Self::Lfo1 => 0,
            Self::Lfo2 => 1,
            Self::AmpEnvelope => 2,
            Self::FilterEnvelope => 3,
            Self::Velocity => 4,
            Self::ModWheel => 5,
            Self::Aftertouch => 6,
            Self::PitchBend => 7,
        }
    }
}

/// Modulation destination identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ModDestination {
    /// Oscillator pitch (in semitones).
    Pitch,
    /// Filter cutoff frequency (in octaves).
    FilterCutoff,
    /// Filter resonance.
    FilterResonance,
    /// Output amplitude / volume.
    Amplitude,
    /// Stereo pan position.
    Pan,
    /// Pulse width (for pulse waveform).
    PulseWidth,
    /// FM modulation index.
    FmIndex,
    /// LFO rate.
    LfoRate,
}

impl ModDestination {
    /// Map destination to array index (decoupled from enum discriminant).
    #[inline]
    fn index(self) -> usize {
        match self {
            Self::Pitch => 0,
            Self::FilterCutoff => 1,
            Self::FilterResonance => 2,
            Self::Amplitude => 3,
            Self::Pan => 4,
            Self::PulseWidth => 5,
            Self::FmIndex => 6,
            Self::LfoRate => 7,
        }
    }
}

/// A single modulation routing: source → destination with depth.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ModRouting {
    /// Source of modulation.
    pub source: ModSource,
    /// Destination to modulate.
    pub destination: ModDestination,
    /// Modulation depth (-1.0 to +1.0).
    pub depth: f32,
    /// Whether this routing is active.
    pub enabled: bool,
}

impl ModRouting {
    /// Create a new modulation routing.
    #[must_use]
    pub fn new(source: ModSource, destination: ModDestination, depth: f32) -> Self {
        Self {
            source,
            destination,
            depth: depth.clamp(-1.0, 1.0),
            enabled: true,
        }
    }
}

/// Maximum number of modulation routings.
pub const MAX_ROUTINGS: usize = 16;

/// Modulation matrix with up to 16 routing slots.
///
/// Consumers provide source values each frame, then query the accumulated
/// modulation for each destination. The matrix does not own the sources —
/// it simply maps input values to output modulation amounts.
///
/// # Usage
///
/// ```rust
/// use naad::mod_matrix::*;
///
/// let mut matrix = ModMatrix::new();
/// matrix.add_routing(ModRouting::new(ModSource::Lfo1, ModDestination::Pitch, 0.5));
///
/// // Each frame: set source values, then compute destinations
/// matrix.set_source(ModSource::Lfo1, 0.7); // LFO is at 0.7
/// matrix.compute();
///
/// let pitch_mod = matrix.get_destination(ModDestination::Pitch);
/// // pitch_mod = 0.7 * 0.5 = 0.35
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModMatrix {
    /// Routing slots.
    routings: Vec<ModRouting>,
    /// Source values for the current frame.
    #[serde(skip)]
    source_values: [f32; NUM_SOURCES],
    /// Computed destination modulation amounts.
    #[serde(skip)]
    destination_values: [f32; NUM_DESTINATIONS],
}

impl ModMatrix {
    /// Create an empty modulation matrix.
    #[must_use]
    pub fn new() -> Self {
        Self {
            routings: Vec::new(),
            source_values: [0.0; 8],
            destination_values: [0.0; 8],
        }
    }

    /// Add a modulation routing. Returns false if matrix is full.
    pub fn add_routing(&mut self, routing: ModRouting) -> bool {
        if self.routings.len() >= MAX_ROUTINGS {
            return false;
        }
        self.routings.push(routing);
        true
    }

    /// Remove a routing by index.
    pub fn remove_routing(&mut self, index: usize) {
        if index < self.routings.len() {
            self.routings.remove(index);
        }
    }

    /// Clear all routings.
    pub fn clear(&mut self) {
        self.routings.clear();
    }

    /// Set a source value for the current frame.
    pub fn set_source(&mut self, source: ModSource, value: f32) {
        self.source_values[source.index()] = value;
    }

    /// Compute all destination modulation values from current sources.
    ///
    /// Call this once per frame after setting all source values.
    pub fn compute(&mut self) {
        self.destination_values = [0.0; 8];
        for routing in &self.routings {
            if !routing.enabled {
                continue;
            }
            let src_val = self.source_values[routing.source.index()];
            let mod_amount = src_val * routing.depth;
            self.destination_values[routing.destination.index()] += mod_amount;
        }
    }

    /// Get the total modulation amount for a destination.
    ///
    /// Returns 0.0 if no routings target this destination.
    #[inline]
    #[must_use]
    pub fn get_destination(&self, dest: ModDestination) -> f32 {
        self.destination_values[dest.index()]
    }

    /// Returns the number of active routings.
    #[must_use]
    pub fn num_routings(&self) -> usize {
        self.routings.len()
    }

    /// Returns a slice of all routings.
    #[must_use]
    pub fn routings(&self) -> &[ModRouting] {
        &self.routings
    }

    /// Get a mutable reference to a routing by index.
    pub fn routing_mut(&mut self, index: usize) -> Option<&mut ModRouting> {
        self.routings.get_mut(index)
    }
}

impl Default for ModMatrix {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_routing() {
        let mut m = ModMatrix::new();
        m.add_routing(ModRouting::new(ModSource::Lfo1, ModDestination::Pitch, 0.5));
        m.set_source(ModSource::Lfo1, 0.8);
        m.compute();
        let val = m.get_destination(ModDestination::Pitch);
        assert!((val - 0.4).abs() < 0.001, "0.8 * 0.5 = 0.4, got {val}");
    }

    #[test]
    fn test_multiple_sources_same_dest() {
        let mut m = ModMatrix::new();
        m.add_routing(ModRouting::new(
            ModSource::Lfo1,
            ModDestination::FilterCutoff,
            0.5,
        ));
        m.add_routing(ModRouting::new(
            ModSource::FilterEnvelope,
            ModDestination::FilterCutoff,
            1.0,
        ));
        m.set_source(ModSource::Lfo1, 0.6);
        m.set_source(ModSource::FilterEnvelope, 0.4);
        m.compute();
        let val = m.get_destination(ModDestination::FilterCutoff);
        // 0.6*0.5 + 0.4*1.0 = 0.3 + 0.4 = 0.7
        assert!((val - 0.7).abs() < 0.001, "expected 0.7, got {val}");
    }

    #[test]
    fn test_disabled_routing() {
        let mut m = ModMatrix::new();
        let mut r = ModRouting::new(ModSource::Lfo1, ModDestination::Pitch, 1.0);
        r.enabled = false;
        m.add_routing(r);
        m.set_source(ModSource::Lfo1, 1.0);
        m.compute();
        assert!(
            m.get_destination(ModDestination::Pitch).abs() < f32::EPSILON,
            "disabled routing should not contribute"
        );
    }

    #[test]
    fn test_max_routings() {
        let mut m = ModMatrix::new();
        for _ in 0..MAX_ROUTINGS {
            assert!(m.add_routing(ModRouting::new(ModSource::Lfo1, ModDestination::Pitch, 0.1)));
        }
        // 17th should fail
        assert!(!m.add_routing(ModRouting::new(ModSource::Lfo1, ModDestination::Pitch, 0.1)));
    }

    #[test]
    fn test_serde_roundtrip() {
        let mut m = ModMatrix::new();
        m.add_routing(ModRouting::new(
            ModSource::Velocity,
            ModDestination::Amplitude,
            0.8,
        ));
        let json = serde_json::to_string(&m).unwrap();
        let back: ModMatrix = serde_json::from_str(&json).unwrap();
        assert_eq!(m.num_routings(), back.num_routings());
    }
}
