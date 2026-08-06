//! Host transport snapshot for one process block.

/// Host timeline state, mapped from CLAP `clap_event_transport`
/// (VST3/LV2 map onto the same shape later).
#[derive(Clone, Copy, Debug, Default)]
pub struct Transport {
    pub playing: bool,
    pub recording: bool,
    pub loop_active: bool,
    /// BPM, when the host provides a tempo.
    pub tempo: Option<f64>,
    /// Song position in quarter notes (beats timeline).
    pub position_beats: Option<f64>,
    /// Song position in seconds (seconds timeline).
    pub position_seconds: Option<f64>,
    /// Loop range in beats (beats timeline).
    pub loop_beats: Option<(f64, f64)>,
    /// Time signature (numerator, denominator).
    pub time_signature: Option<(u16, u16)>,
    /// Current bar number (beats timeline).
    pub bar_number: Option<i32>,
}
