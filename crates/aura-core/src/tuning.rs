//! Host tuning support (CLAP `clap.tuning` / MTS-ESP).
//!
//! Format wrappers that support dynamic tuning inject a [`TuningProvider`]
//! into [`crate::ProcessContext`]. Plugins call [`Tuning::relative_offset`]
//! to obtain the detune in semitones for a given key and
//! [`Tuning::should_play`] to honor host-driven note filtering.

use std::sync::Arc;

/// A tuning selection event delivered by the host.
///
/// `port_index == -1` and `channel == -1` are global wildcards. More specific
/// events override broader ones when the provider resolves a lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TuningEvent {
    pub sample_offset: u32,
    pub port_index: i16,
    pub channel: i16,
    pub tuning_id: u32,
}

/// Backing implementation for [`Tuning`], supplied by the format wrapper.
pub trait TuningProvider: Send + Sync {
    /// Relative tuning in semitones against equal temperament (A4 = 440 Hz).
    fn relative_offset(&self, port_index: i32, channel: i32, key: i32, sample_offset: u32) -> f64;

    /// Returns `false` when the host wants the note suppressed.
    fn should_play(&self, port_index: i32, channel: i32, key: i32) -> bool;

    /// Apply a tuning selection event (update the active tuning id).
    fn apply(&self, _event: &TuningEvent) {}
}

/// Default no-op provider: equal temperament, all notes play.
#[derive(Debug, Clone, Copy)]
struct NullTuning;

impl TuningProvider for NullTuning {
    fn relative_offset(
        &self,
        _port_index: i32,
        _channel: i32,
        _key: i32,
        _sample_offset: u32,
    ) -> f64 {
        0.0
    }

    fn should_play(&self, _port_index: i32, _channel: i32, _key: i32) -> bool {
        true
    }
}

/// Handle passed to [`crate::ProcessContext`] for per-voice tuning queries.
#[derive(Clone)]
pub struct Tuning {
    provider: Arc<dyn TuningProvider>,
}

impl Default for Tuning {
    fn default() -> Self {
        Self::disabled()
    }
}

impl Tuning {
    /// Create a tuning handle backed by `provider`.
    #[must_use]
    pub fn new(provider: Arc<dyn TuningProvider>) -> Self {
        Self { provider }
    }

    /// Equal temperament fallback; all notes play with zero offset.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            provider: Arc::new(NullTuning),
        }
    }

    /// Apply a host tuning-selection event.
    pub fn apply_event(&self, event: &TuningEvent) {
        self.provider.apply(event);
    }

    /// Relative tuning in semitones for `(port, channel, key)` at the given
    /// sample offset within the current block.
    #[must_use]
    pub fn relative_offset(
        &self,
        port_index: i32,
        channel: i32,
        key: i32,
        sample_offset: u32,
    ) -> f64 {
        self.provider
            .relative_offset(port_index, channel, key, sample_offset)
    }

    /// Returns `false` when the host wants the note suppressed.
    #[must_use]
    pub fn should_play(&self, port_index: i32, channel: i32, key: i32) -> bool {
        self.provider.should_play(port_index, channel, key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::float_cmp)]
    fn disabled_is_zero_and_plays() {
        let t = Tuning::disabled();
        assert_eq!(t.relative_offset(0, 0, 60, 0), 0.0);
        assert!(t.should_play(0, 0, 60));
    }
}
