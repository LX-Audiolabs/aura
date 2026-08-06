//! Activation-time processing configuration.

/// How the host is driving audio through the plugin this activation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ProcessMode {
    /// Fixed-rate realtime playback. Honor the no-alloc / no-lock rule.
    #[default]
    Realtime,
    /// Ahead-of-time / prefetch style (e.g. VST3 kPrefetch). Treat like realtime
    /// unless you have a specific reason to relax discipline.
    Buffered,
    /// Freewheeling offline render — no wall-clock deadline.
    Offline,
}

impl ProcessMode {
    #[must_use]
    pub fn is_offline(self) -> bool {
        matches!(self, Self::Offline)
    }
}

/// Config handed to `reset` when the host (re)prepares the plugin.
#[derive(Clone, Copy, Debug)]
pub struct AudioConfig {
    pub sample_rate: f64,
    pub max_block_size: usize,
    pub process_mode: ProcessMode,
}

impl AudioConfig {
    #[must_use]
    pub fn new(sample_rate: f64, max_block_size: usize) -> Self {
        Self {
            sample_rate,
            max_block_size,
            process_mode: ProcessMode::Realtime,
        }
    }

    #[must_use]
    pub fn with_process_mode(mut self, mode: ProcessMode) -> Self {
        self.process_mode = mode;
        self
    }
}
