//! Host tuning support (CLAP `clap.tuning` / MTS-ESP).
//!
//! Format wrappers that support dynamic tuning inject a [`TuningProvider`]
//! into [`crate::ProcessContext`]. Plugins call [`Tuning::relative_offset`]
//! to obtain the detune in semitones for a given key and
//! [`Tuning::should_play`] to honor host-driven note filtering.

use std::sync::Arc;

/// Metadata for one host tuning entry (CLAP `clap.tuning/2`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TuningInfo {
    /// Host-side tuning id.
    pub id: u32,
    /// Human-readable tuning name.
    pub name: String,
    /// `true` when the tuning may change over time.
    pub is_dynamic: bool,
}

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

    /// Number of tunings exposed by the host. `0` if unsupported or empty.
    fn tuning_count(&self) -> u32 {
        0
    }

    /// Metadata for the tuning at `index`. `None` if out of range or unsupported.
    fn tuning_info(&self, _index: u32) -> Option<TuningInfo> {
        None
    }
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

    fn tuning_count(&self) -> u32 {
        0
    }

    fn tuning_info(&self, _index: u32) -> Option<TuningInfo> {
        None
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

    /// Number of tunings exposed by the host.
    #[must_use]
    pub fn tuning_count(&self) -> u32 {
        self.provider.tuning_count()
    }

    /// Metadata for the tuning at `index`.
    #[must_use]
    pub fn tuning_info(&self, index: u32) -> Option<TuningInfo> {
        self.provider.tuning_info(index)
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

    #[test]
    fn disabled_has_no_tunings() {
        let t = Tuning::disabled();
        assert_eq!(t.tuning_count(), 0);
        assert!(t.tuning_info(0).is_none());
    }
}
