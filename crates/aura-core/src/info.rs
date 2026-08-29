//! Static plugin metadata (CLAP / VST3 / LV2 focused — no AU/AAX fields).

/// Wire dialect a MIDI port speaks.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum MidiDialect {
    #[default]
    Midi1,
    /// Advertise MIDI 2 on the note port. Process sees native packets on
    /// [`crate::ProcessContext::ump`]; a 7-bit image is still mirrored to `midi`.
    Midi2,
    /// Prefer native CLAP notes (`CLAP_NOTE_DIALECT_CLAP`) so hosts send
    /// `CLAP_EVENT_NOTE_*` + note expressions + per-note `PARAM_MOD`.
    /// MIDI 1/2 stay supported; `ump` / `midi` are still filled.
    Clap,
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
/// Format wrappers read this for registration. IDs for formats we do not
/// ship (AU fourcc, AAX) are omitted on purpose.
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
    /// Voices the patch may use. `0` = do not advertise `clap.voice-info`.
    /// Bitwig Voice Stack needs this (`voice_count > 1`) plus overlapping notes.
    pub voice_count: u32,
    /// Allocated voice pool (`voice_count <= voice_capacity`). `0` → same as count.
    pub voice_capacity: u32,
    /// Advertise CLAP `clap.tuning/2` (MTS-ESP / dynamic tuning host support).
    pub supports_tuning: bool,
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
            voice_count: 0,
            voice_capacity: 0,
            supports_tuning: false,
        }
    }
}
