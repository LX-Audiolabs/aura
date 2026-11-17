//! User-facing plugin logic (minimal f32 leaf).

use std::sync::Arc;

use aura_params::Params;

use crate::buffer::AudioBuffer;
use crate::bus::BusLayout;
use crate::config::AudioConfig;
use crate::editor::Editor;
use crate::info::PluginInfo;
use crate::preset::FactoryPreset;
use crate::process::{ProcessContext, ProcessStatus};
use crate::state::StateContext;

/// One entry in a plugin's custom note-name table (`CLAP_EXT_NOTE_NAME`).
///
/// Set `port`, `channel`, or `key` to `-1` to match any value (wildcard).
pub struct NoteNameEntry {
    pub port: i16,
    pub channel: i16,
    pub key: i16,
    pub name: &'static str,
}

/// One curated CLAP remote-controls page. Hosts may persist `page_id`, so keep
/// it stable. Missing parameter slots remain unassigned.
///
/// `param_ids` lists at most 8 entries (CLAP remote-controls slot count);
/// format wrappers drop extras.
pub struct RemoteControlsPage {
    pub page_id: u32,
    pub section_name: &'static str,
    pub page_name: &'static str,
    pub param_ids: &'static [u32],
    pub is_for_preset: bool,
}

/// One host context-menu action supplied by a plugin.
pub struct ContextMenuEntry {
    pub action_id: u32,
    pub label: &'static str,
    pub is_enabled: bool,
}

/// What plugin authors implement for realtime DSP + optional GUI factory.
///
/// Format wrappers (CLAP/VST3/LV2) call these methods.
pub trait PluginLogic: 'static {
    type Params: Params + Default;

    /// Mutable per-instance DSP state. Owned by the shell, not `Self`.
    type DspState: Send + 'static;

    /// Static plugin metadata (CLAP id, vendor, …).
    fn info() -> PluginInfo;

    /// Supported bus layouts (first entry is the default).
    ///
    /// Override with [`BusLayout::mono`], [`BusLayout::stereo_and_mono`],
    /// `.with_sidechain` / `.with_aux` as needed. Process must tolerate every
    /// declared width (loop over buffer channels; do not hardcode 2).
    ///
    /// **LV2** uses only the first entry (no runtime layout switch).
    #[must_use]
    fn bus_layouts() -> Vec<BusLayout> {
        vec![BusLayout::stereo()]
    }

    /// Build initial DSP state.
    fn init(params: &Self::Params, sample_rate: f64) -> Self::DspState;

    /// Host (re)prepare — allocate / clear DSP for the new config.
    fn reset(state: &mut Self::DspState, params: &Self::Params, config: &AudioConfig);

    /// Audio thread process.
    ///
    /// **Automation timing differs by format:** CLAP applies `CHUNKED` params
    /// sample-accurately (block splits). VST3 applies the last point per block.
    /// LV2 updates from control ports once per `run`. Write DSP that tolerates
    /// both, or document format-specific behavior in the product.
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

    /// Reporting delay in samples (PDC). Default `0` (zero-latency FX).
    ///
    /// Hosts read this via CLAP `clap.latency` / VST3 `getLatencySamples`.
    /// If the value changes after activation, format wrappers notify the host
    /// (restart / latency-changed) so delay compensation stays honest.
    #[must_use]
    fn latency(_state: &Self::DspState) -> u32 {
        0
    }

    /// Processing tail length in samples after input goes silent.
    ///
    /// Hosts read this via CLAP `clap.tail` / VST3 `getTailSamples`.
    /// Default `0` (no tail). Return a large value (e.g. `u32::MAX`) when
    /// the tail is effectively infinite / unknown.
    #[must_use]
    fn tail_length(_state: &Self::DspState) -> u32 {
        0
    }

    /// Bundled factory presets for CLAP host browsers (`preset-discovery`
    /// + `preset-load` PLUGIN location). Empty = no discovery factory.
    #[must_use]
    fn factory_presets() -> &'static [FactoryPreset] {
        &[]
    }

    /// Load a host-chosen preset file (CLAP `preset-load` FILE location).
    /// Default: shared host blob ([`crate::decode_state`], v1 or v2).
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or is not a valid state blob.
    fn load_preset_from_file(params: &Self::Params, path: &std::path::Path) -> Result<(), String> {
        crate::preset::load_preset_file(params, path)
    }

    /// Host tuning pool changed (`clap.tuning`). Audio thread, next block.
    /// **CLAP-oriented**; VST3/LV2 do not call this. Default: no-op.
    fn tuning_changed(_state: &mut Self::DspState, _params: &Self::Params) {}

    /// Custom note names (`CLAP_EXT_NOTE_NAME`). Non-empty activates the
    /// extension. **CLAP-oriented**; unused on VST3/LV2. Default: empty.
    #[must_use]
    fn note_names() -> &'static [NoteNameEntry] {
        &[]
    }

    /// Curated CLAP remote-controls pages. `None` keeps generated group pages;
    /// `Some` replaces them, including `Some(&[])` to expose no pages.
    #[must_use]
    fn remote_control_pages() -> Option<&'static [RemoteControlsPage]> {
        None
    }

    /// Hardware control mapping changed (`CLAP_EXT_PARAM_INDICATION`
    /// `set_mapping`). Main thread. **CLAP-oriented**. Default: no-op.
    fn on_param_mapping(_params: &Self::Params, _param_id: u32, _has_mapping: bool) {}

    /// Automation indication changed (`CLAP_EXT_PARAM_INDICATION`
    /// `set_automation`). Main thread. **CLAP-oriented**. Default: no-op.
    fn on_param_automation(_params: &Self::Params, _param_id: u32, _automation_state: u32) {}

    /// Opt into CLAP state-context callbacks. Default preserves `clap.state` only.
    #[must_use]
    fn state_context_supported() -> bool {
        false
    }

    /// Encode state for a CLAP preset, duplicate, or project operation.
    fn save_state_with_context(params: &Self::Params, _context: StateContext) -> Vec<u8> {
        crate::encode_state(params)
    }

    /// Restore state saved for any CLAP state context.
    fn load_state_with_context(params: &Self::Params, _context: StateContext, blob: &[u8]) -> bool {
        crate::decode_state(params, blob)
    }

    /// Opt into CLAP parameter context-menu entries.
    #[must_use]
    fn context_menu_supported() -> bool {
        false
    }

    /// Menu entries for a parameter, or `None` for the global plugin menu.
    #[must_use]
    fn context_menu_entries(_param_id: Option<u32>) -> &'static [ContextMenuEntry] {
        &[]
    }

    /// Handle a previously advertised context-menu action on the main thread.
    fn on_context_menu_action(
        _params: &Self::Params,
        _param_id: Option<u32>,
        _action_id: u32,
    ) -> bool {
        false
    }
}
