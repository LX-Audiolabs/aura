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
    /// Main input channel count for the selected bus layout (`0` = no input).
    pub main_input_channels: u32,
    /// Main output channel count for the selected bus layout.
    pub main_output_channels: u32,
    /// Optional sidechain input channel count for the selected bus layout (`0` = none).
    pub sidechain_input_channels: u32,
    /// Optional aux output channel count for the selected bus layout (`0` = none).
    pub aux_output_channels: u32,
}

impl AudioConfig {
    /// Stereo main I/O defaults (matches [`BusLayout::stereo`](crate::BusLayout::stereo)).
    #[must_use]
    pub fn new(sample_rate: f64, max_block_size: usize) -> Self {
        Self {
            sample_rate,
            max_block_size,
            process_mode: ProcessMode::Realtime,
            main_input_channels: 2,
            main_output_channels: 2,
            sidechain_input_channels: 0,
            aux_output_channels: 0,
        }
    }

    #[must_use]
    pub fn with_process_mode(mut self, mode: ProcessMode) -> Self {
        self.process_mode = mode;
        self
    }

    #[must_use]
    pub fn with_channels(mut self, main_in: u32, main_out: u32) -> Self {
        self.main_input_channels = main_in;
        self.main_output_channels = main_out;
        self
    }

    #[must_use]
    pub fn with_sidechain_channels(mut self, sidechain_in: u32) -> Self {
        self.sidechain_input_channels = sidechain_in;
        self
    }

    #[must_use]
    pub fn with_aux_channels(mut self, aux_out: u32) -> Self {
        self.aux_output_channels = aux_out;
        self
    }
}
