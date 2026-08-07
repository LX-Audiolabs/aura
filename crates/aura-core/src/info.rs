//! Static plugin metadata (CLAP / VST3 / LV2 focused — no AU/AAX fields).

/// Wire dialect a MIDI port speaks.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum MidiDialect {
    #[default]
    Midi1,
    /// Architecture reserved; full UMP later.
    Midi2,
}

/// Broad plugin category for host browsers.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum PluginCategory {
    #[default]
    Effect,
    Instrument,
    Analyzer,
    NoteEffect,
}

/// Static metadata about a plugin.
///
/// Format wrappers (later) read this for registration. IDs that only
/// apply to formats we do not ship (AU fourcc, AAX) are omitted on purpose.
#[derive(Clone, Debug)]
pub struct PluginInfo {
    pub name: &'static str,
    pub vendor: &'static str,
    pub url: &'static str,
    pub version: &'static str,
    pub category: PluginCategory,
    /// Short id (`bundle_id` / clap-friendly slug).
    pub bundle_id: &'static str,
    pub clap_id: &'static str,
    pub vst3_id: &'static str,
    /// LV2 plugin URI (RFC 3986). Empty → `https://lx-audiolabs.com/lv2/<bundle_id>`.
    pub lv2_uri: &'static str,
    pub accepts_midi_in: bool,
    pub emits_midi: bool,
    pub midi_input_dialect: MidiDialect,
    pub midi_output_dialect: MidiDialect,
}

impl PluginInfo {
    /// Minimal constructor — fill format IDs later via struct update.
    #[must_use]
    pub const fn new(
        name: &'static str,
        vendor: &'static str,
        version: &'static str,
        bundle_id: &'static str,
    ) -> Self {
        Self {
            name,
            vendor,
            url: "",
            version,
            category: PluginCategory::Effect,
            bundle_id,
            clap_id: bundle_id,
            vst3_id: "",
            lv2_uri: "",
            accepts_midi_in: false,
            emits_midi: false,
            midi_input_dialect: MidiDialect::Midi1,
            midi_output_dialect: MidiDialect::Midi1,
        }
    }
}
