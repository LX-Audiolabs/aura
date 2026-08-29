//! Main-bus channel layouts for format wrappers.
//!
//! Plugins declare supported layouts via [`PluginLogic::bus_layouts`](crate::PluginLogic::bus_layouts).
//! Default is stereo in/out. Mono-only or dual mono+stereo is opt-in.
//! One optional sidechain input and one optional aux output can be declared per layout.

/// Channel width of a main audio bus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChannelConfig {
    Mono,
    Stereo,
}

impl ChannelConfig {
    #[must_use]
    pub const fn channel_count(self) -> u32 {
        match self {
            Self::Mono => 1,
            Self::Stereo => 2,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Mono => "Mono",
            Self::Stereo => "Stereo",
        }
    }
}

/// One complete I/O layout (main in/out + optional sidechain in + optional aux out).
///
/// Instruments can use [`Self::output_only`]. Effects use [`Self::mono`] /
/// [`Self::stereo`] / [`Self::stereo_and_mono`]. Add buses via
/// [`Self::with_sidechain`] / [`Self::with_aux`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BusLayout {
    /// Main input width. `None` = no audio input (generator / instrument).
    pub main_in: Option<ChannelConfig>,
    /// Main output width.
    pub main_out: ChannelConfig,
    /// Optional sidechain input bus. `None` = no sidechain.
    pub sidechain_in: Option<ChannelConfig>,
    /// Optional aux output bus. `None` = no aux out.
    pub aux_out: Option<ChannelConfig>,
}

impl BusLayout {
    #[must_use]
    pub const fn mono() -> Self {
        Self {
            main_in: Some(ChannelConfig::Mono),
            main_out: ChannelConfig::Mono,
            sidechain_in: None,
            aux_out: None,
        }
    }

    #[must_use]
    pub const fn stereo() -> Self {
        Self {
            main_in: Some(ChannelConfig::Stereo),
            main_out: ChannelConfig::Stereo,
            sidechain_in: None,
            aux_out: None,
        }
    }

    /// Output-only layout (instrument / generator).
    #[must_use]
    pub const fn output_only(out: ChannelConfig) -> Self {
        Self {
            main_in: None,
            main_out: out,
            sidechain_in: None,
            aux_out: None,
        }
    }

    /// Return a copy of this layout with the given sidechain input bus.
    #[must_use]
    pub const fn with_sidechain(self, sidechain: ChannelConfig) -> Self {
        Self {
            sidechain_in: Some(sidechain),
            ..self
        }
    }

    /// Return a copy of this layout with the given aux output bus.
    #[must_use]
    pub const fn with_aux(self, aux: ChannelConfig) -> Self {
        Self {
            aux_out: Some(aux),
            ..self
        }
    }

    /// Default effect set: stereo first (host default), then mono.
    #[must_use]
    pub fn stereo_and_mono() -> Vec<Self> {
        vec![Self::stereo(), Self::mono()]
    }

    #[must_use]
    pub const fn main_input_channels(self) -> u32 {
        match self.main_in {
            Some(c) => c.channel_count(),
            None => 0,
        }
    }

    #[must_use]
    pub const fn sidechain_input_channels(self) -> u32 {
        match self.sidechain_in {
            Some(c) => c.channel_count(),
            None => 0,
        }
    }

    #[must_use]
    pub const fn aux_output_channels(self) -> u32 {
        match self.aux_out {
            Some(c) => c.channel_count(),
            None => 0,
        }
    }

    /// CLAP/VST3 input *port* count (main + optional sidechain), not channels.
    ///
    /// Stereo sidechain is still one port with two channels. Using
    /// [`Self::sidechain_input_channels`] here advertises extra ports with
    /// duplicate ids and breaks host routing (Bitwig dry-passthrough).
    #[must_use]
    pub const fn input_port_count(self) -> u32 {
        let main = if self.main_in.is_some() { 1 } else { 0 };
        let sc = if self.sidechain_in.is_some() { 1 } else { 0 };
        main + sc
    }

    /// CLAP/VST3 output *port* count (main + optional aux), not channels.
    #[must_use]
    pub const fn output_port_count(self) -> u32 {
        if self.aux_out.is_some() { 2 } else { 1 }
    }

    /// Total audio input channels passed to [`AudioBuffer`](crate::AudioBuffer):
    /// main inputs followed by sidechain inputs.
    #[must_use]
    pub const fn total_input_channels(self) -> u32 {
        self.main_input_channels() + self.sidechain_input_channels()
    }

    #[must_use]
    pub const fn main_output_channels(self) -> u32 {
        self.main_out.channel_count()
    }

    /// Total audio output channels: main outputs followed by aux outputs.
    #[must_use]
    pub const fn total_output_channels(self) -> u32 {
        self.main_output_channels() + self.aux_output_channels()
    }

    /// Human-readable config name for host menus (`clap.audio-ports-config`).
    #[must_use]
    pub fn config_name(self) -> String {
        let base = match self.main_in {
            Some(inn) if inn == self.main_out => inn.name().to_string(),
            Some(inn) => format!("{} in / {} out", inn.name(), self.main_out.name()),
            None => format!("{} out", self.main_out.name()),
        };
        let with_sc = match self.sidechain_in {
            Some(sc) => format!("{base} + {} sidechain", sc.name()),
            None => base,
        };
        match self.aux_out {
            Some(aux) => format!("{with_sc} + {} aux", aux.name()),
            None => with_sc,
        }
    }
}

/// Resolve a layout by index, defaulting to the first entry or stereo.
#[must_use]
pub fn layout_at(layouts: &[BusLayout], index: usize) -> BusLayout {
    layouts
        .get(index)
        .copied()
        .or_else(|| layouts.first().copied())
        .unwrap_or_else(BusLayout::stereo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_stereo_channel_counts() {
        assert_eq!(BusLayout::mono().main_input_channels(), 1);
        assert_eq!(BusLayout::mono().main_output_channels(), 1);
        assert_eq!(BusLayout::stereo().main_input_channels(), 2);
        assert_eq!(BusLayout::stereo().main_output_channels(), 2);
        assert_eq!(
            BusLayout::output_only(ChannelConfig::Stereo).main_input_channels(),
            0
        );
    }

    #[test]
    fn stereo_and_mono_order() {
        let v = BusLayout::stereo_and_mono();
        assert_eq!(v, vec![BusLayout::stereo(), BusLayout::mono()]);
    }

    #[test]
    fn layout_at_falls_back() {
        assert_eq!(layout_at(&[], 0), BusLayout::stereo());
        let v = BusLayout::stereo_and_mono();
        assert_eq!(layout_at(&v, 1), BusLayout::mono());
        assert_eq!(layout_at(&v, 99), BusLayout::stereo());
    }

    #[test]
    fn sidechain_input_counts() {
        let sc = BusLayout::stereo().with_sidechain(ChannelConfig::Mono);
        assert_eq!(sc.main_input_channels(), 2);
        assert_eq!(sc.sidechain_input_channels(), 1);
        assert_eq!(sc.total_input_channels(), 3);
        assert_eq!(sc.main_output_channels(), 2);
        assert_eq!(sc.input_port_count(), 2);
        assert_eq!(sc.output_port_count(), 1);
        assert!(sc.config_name().contains("Mono sidechain"));
    }

    #[test]
    fn stereo_sidechain_is_two_input_ports() {
        let sc = BusLayout::stereo().with_sidechain(ChannelConfig::Stereo);
        assert_eq!(sc.sidechain_input_channels(), 2);
        assert_eq!(sc.total_input_channels(), 4);
        assert_eq!(sc.input_port_count(), 2);
    }

    #[test]
    fn aux_output_counts() {
        let aux = BusLayout::stereo().with_aux(ChannelConfig::Stereo);
        assert_eq!(aux.main_output_channels(), 2);
        assert_eq!(aux.aux_output_channels(), 2);
        assert_eq!(aux.total_output_channels(), 4);
        assert_eq!(aux.output_port_count(), 2);
        assert!(aux.config_name().contains("Stereo aux"));
    }

    #[test]
    fn aux_and_sidechain() {
        let both = BusLayout::stereo()
            .with_sidechain(ChannelConfig::Mono)
            .with_aux(ChannelConfig::Mono);
        assert_eq!(both.input_port_count(), 2);
        assert_eq!(both.output_port_count(), 2);
        assert_eq!(both.total_input_channels(), 3);
        assert_eq!(both.total_output_channels(), 3);
    }
}
