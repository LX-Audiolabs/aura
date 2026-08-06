//! Per-block process context (minimal).

use crate::config::ProcessMode;

/// What the plugin wants after `process`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ProcessStatus {
    /// Keep calling process (normal).
    #[default]
    Continue,
    /// No more output expected until something changes (tail finished).
    TailFinished,
    /// Error — host may suspend.
    Error,
}

/// Per-block context handed to `process()`.
///
/// Expanded later (transport, events, meters, tasks). For now: rate + size.
#[non_exhaustive]
pub struct ProcessContext {
    pub sample_rate: f64,
    pub block_size: usize,
    pub process_mode: ProcessMode,
}

impl ProcessContext {
    #[must_use]
    pub fn new(sample_rate: f64, block_size: usize) -> Self {
        Self {
            sample_rate,
            block_size,
            process_mode: ProcessMode::Realtime,
        }
    }

    #[must_use]
    pub fn with_process_mode(mut self, mode: ProcessMode) -> Self {
        self.process_mode = mode;
        self
    }
}
