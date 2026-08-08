//! Main-bus channel layouts for format wrappers.
//!
//! Plugins declare supported layouts via [`PluginLogic::bus_layouts`](crate::PluginLogic::bus_layouts).
//! Default is stereo in/out. Mono-only or dual mono+stereo is opt-in.
//! Sidechain / multi-bus is out of scope here (add later when a pilot needs it).

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

/// One complete main-bus I/O layout (single main in + main out).
///
/// Instruments can use [`Self::output_only`]. Effects use [`Self::mono`] /
/// [`Self::stereo`] / [`Self::stereo_and_mono`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BusLayout {
    /// Main input width. `None` = no audio input (generator / instrument).
    pub main_in: Option<ChannelConfig>,
    /// Main output width.
    pub main_out: ChannelConfig,
}

impl BusLayout {
    #[must_use]
    pub const fn mono() -> Self {
        Self {
            main_in: Some(ChannelConfig::Mono),
            main_out: ChannelConfig::Mono,
        }
    }

    #[must_use]
    pub const fn stereo() -> Self {
        Self {
            main_in: Some(ChannelConfig::Stereo),
            main_out: ChannelConfig::Stereo,
        }
    }

    /// Output-only layout (instrument / generator).
    #[must_use]
    pub const fn output_only(out: ChannelConfig) -> Self {
        Self {
            main_in: None,
            main_out: out,
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
    pub const fn main_output_channels(self) -> u32 {
        self.main_out.channel_count()
    }

    /// Human-readable config name for host menus (`clap.audio-ports-config`).
    #[must_use]
    pub fn config_name(self) -> String {
        match self.main_in {
            Some(inn) if inn == self.main_out => inn.name().to_string(),
            Some(inn) => format!("{} in / {} out", inn.name(), self.main_out.name()),
            None => format!("{} out", self.main_out.name()),
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
}
