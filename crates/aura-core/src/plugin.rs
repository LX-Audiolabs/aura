//! User-facing plugin logic (minimal f32 leaf).

use std::sync::Arc;

use aura_params::Params;

use crate::buffer::AudioBuffer;
use crate::config::AudioConfig;
use crate::editor::Editor;
use crate::info::PluginInfo;
use crate::process::{ProcessContext, ProcessStatus};

/// What plugin authors implement for realtime DSP + optional GUI factory.
///
/// Format wrappers (CLAP/VST3/LV2) call these methods. This is the thin AURA
/// equivalent of truce's `PluginLogic` surface; precision-64 and hot-reload
/// extras land when needed.
pub trait PluginLogic: 'static {
    type Params: Params + Default;

    /// Mutable per-instance DSP state. Owned by the shell, not `Self`.
    type DspState: Send + 'static;

    /// Static plugin metadata (CLAP id, vendor, …).
    fn info() -> PluginInfo;

    /// Build initial DSP state.
    fn init(params: &Self::Params, sample_rate: f64) -> Self::DspState;

    /// Host (re)prepare — allocate / clear DSP for the new config.
    fn reset(state: &mut Self::DspState, params: &Self::Params, config: &AudioConfig);

    /// Audio thread process.
    fn process(
        state: &mut Self::DspState,
        params: &Self::Params,
        buffer: &mut AudioBuffer<'_, f32>,
        context: &mut ProcessContext,
    ) -> ProcessStatus;

    /// Optional GUI. Default: no editor.
    fn editor(_params: Arc<Self::Params>) -> Option<Box<dyn Editor>> {
        None
    }

    /// Whether in-place buffer aliasing is supported. Default false.
    #[must_use]
    fn supports_in_place() -> bool {
        false
    }
}
