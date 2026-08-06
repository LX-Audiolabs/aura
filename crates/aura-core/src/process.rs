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
/// Expanded later (note events, meters, tasks). For now: rate + size +
/// optional host transport.
#[non_exhaustive]
pub struct ProcessContext {
    pub sample_rate: f64,
    pub block_size: usize,
    pub process_mode: ProcessMode,
    /// Host timeline for this block; `None` when the host provides none.
    pub transport: Option<crate::transport::Transport>,
}

impl ProcessContext {
    #[must_use]
    pub fn new(sample_rate: f64, block_size: usize) -> Self {
        Self {
            sample_rate,
            block_size,
            process_mode: ProcessMode::Realtime,
            transport: None,
        }
    }

    #[must_use]
    pub fn with_process_mode(mut self, mode: ProcessMode) -> Self {
        self.process_mode = mode;
        self
    }

    #[must_use]
    pub fn with_transport(mut self, transport: crate::transport::Transport) -> Self {
        self.transport = Some(transport);
        self
    }
}
