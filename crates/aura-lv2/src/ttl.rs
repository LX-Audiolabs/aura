//! Turtle (TTL) generation for an LV2 bundle — written by `cargo aura install --lv2`.

#![allow(
    clippy::must_use_candidate,
    clippy::uninlined_format_args,
    clippy::needless_raw_string_hashes
)]

use aura_core::info::{PluginCategory, PluginInfo};
use aura_core::{BusLayout, ChannelConfig};
use aura_params::{ParamInfo, ParamRange, ParamValueKind};

/// Files to write into `<name>.lv2/` next to the shared library.
pub struct BundleTtl {
    pub manifest: String,
    pub plugin: String,
    /// Relative library filename referenced from `manifest.ttl` (e.g. `smoke_gain.dll`).
    pub binary_name: String,
}

/// Generate `manifest.ttl` + plugin TTL for a main-bus FX with control ports.
///
/// Port layout (must match the runtime wrapper) for **stereo**:
/// - 0/1: audio in L/R · 2/3: audio out L/R · 4..: controls
///
/// For **mono**: 0 in · 1 out · 2..: controls.
///
/// LV2 has no runtime layout switch — `layout` is the static declaration
/// (typically the plugin's first / only `bus_layouts()` entry).
pub fn generate_ttl(
    info: &PluginInfo,
    uri: &str,
    binary_stem: &str,
    params: &[ParamInfo],
) -> BundleTtl {
    generate_ttl_with_layout(info, uri, binary_stem, params, BusLayout::stereo())
}

/// Like [`generate_ttl`] but with an explicit main-bus layout.
pub fn generate_ttl_with_layout(
    info: &PluginInfo,
    uri: &str,
    binary_stem: &str,
    params: &[ParamInfo],
    layout: BusLayout,
) -> BundleTtl {
    generate_ttl_with_layout_and_ui(info, uri, binary_stem, params, layout, false)
}

/// Like [`generate_ttl_with_layout`] but optionally declares an LV2 UI.
#[allow(clippy::too_many_lines)]
pub fn generate_ttl_with_layout_and_ui(
    info: &PluginInfo,
    uri: &str,
    binary_stem: &str,
    params: &[ParamInfo],
    layout: BusLayout,
    has_ui: bool,
) -> BundleTtl {
    let binary_name = {
        #[cfg(target_os = "windows")]
        {
            format!("{binary_stem}.dll")
        }
        #[cfg(target_os = "macos")]
        {
            format!("lib{binary_stem}.dylib")
        }
        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        {
            format!("lib{binary_stem}.so")
        }
    };

    let class = match info.category {
        PluginCategory::Effect => "lv2:Plugin, lv2:AmplifierPlugin",
        PluginCategory::Analyzer => "lv2:Plugin, lv2:AnalyserPlugin",
        PluginCategory::Instrument => "lv2:Plugin, lv2:InstrumentPlugin",
        PluginCategory::NoteEffect => "lv2:Plugin",
    };

    let mut ports = String::new();
    let mut idx = 0usize;
    match layout.main_in {
        Some(ChannelConfig::Mono) => {
            ports.push_str(&audio_port(idx, "in", "Input", true));
            idx += 1;
        }
        Some(ChannelConfig::Stereo) => {
            ports.push_str(&audio_port(idx, "in_l", "Input L", true));
            idx += 1;
            ports.push_str(&audio_port(idx, "in_r", "Input R", true));
            idx += 1;
        }
        None => {}
    }
    match layout.main_out {
        ChannelConfig::Mono => {
            ports.push_str(&audio_port(idx, "out", "Output", false));
            idx += 1;
        }
        ChannelConfig::Stereo => {
            ports.push_str(&audio_port(idx, "out_l", "Output L", false));
            idx += 1;
            ports.push_str(&audio_port(idx, "out_r", "Output R", false));
            idx += 1;
        }
    }
    let ctrl0 = idx;
    for (i, p) in params.iter().enumerate() {
        let port_idx = ctrl0 + i;
        let sym = param_symbol(p);
        ports.push_str(&control_port(port_idx, &sym, p));
    }
    if info.accepts_midi_in {
        // MIDI atom input goes last — must match the runtime wrapper's port map.
        ports.push_str(&midi_in_port(ctrl0 + params.len()));
    }

    let io_note = layout.config_name();
    let ui_block = if has_ui {
        format!(
            r#"
<{uri}#ui>
    a ui:X11UI, ui:WindowsUI, ui:CocoaUI ;
    ui:binary <{binary}> ;
    lv2:extensionData ui:idleInterface ."#,
            uri = uri,
            binary = binary_name,
        )
    } else {
        String::new()
    };
    let ui_ref = if has_ui {
        format!("\n    ui:ui <{uri}#ui> ;", uri = uri)
    } else {
        String::new()
    };

    let plugin = format!(
        r#"@prefix doap:  <http://usefulinc.com/ns/doap#> .
@prefix foaf:  <http://xmlns.com/foaf/0.1/> .
@prefix lv2:   <http://lv2plug.in/ns/lv2core#> .
@prefix atom:  <http://lv2plug.in/ns/ext/atom#> .
@prefix midi:  <http://lv2plug.in/ns/ext/midi#> .
@prefix rdfs:  <http://www.w3.org/2000/01/rdf-schema#> .
@prefix unit:  <http://lv2plug.in/ns/extensions/units#> .
@prefix state: <http://lv2plug.in/ns/ext/state#> .
@prefix ui:    <http://lv2plug.in/ns/extensions/ui#> .

<{uri}>
    a {class} ;
    doap:name "{name}" ;
    doap:license <https://spdx.org/licenses/GPL-3.0-or-later> ;
    lv2:project <https://lx-audiolabs.com/> ;
    lv2:optionalFeature lv2:hardRTCapable, state:loadDefaultState ;
    lv2:extensionData state:interface ;{ui_ref}
    rdfs:comment "AURA LV2 wrapper — {io_note}, control ports for params." ;
{ports}
    .
{ui_block}
"#,
        uri = uri,
        class = class,
        name = escape_ttl(info.name),
        io_note = escape_ttl(&io_note),
        ports = ports,
        ui_ref = ui_ref,
        ui_block = ui_block,
    );

    // Vendor as foaf:maker optional — keep TTL short.

    let manifest = format!(
        r#"@prefix lv2:  <http://lv2plug.in/ns/lv2core#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .

<{uri}>
    a lv2:Plugin ;
    lv2:binary <{binary}> ;
    rdfs:seeAlso <plugin.ttl> .
"#,
        uri = uri,
        binary = binary_name,
    );

    BundleTtl {
        manifest,
        plugin,
        binary_name,
    }
}

fn param_symbol(p: &ParamInfo) -> String {
    // LV2 symbols: [a-zA-Z_][a-zA-Z0-9_]*
    let mut s = format!("p_{}", p.id);
    if !p.short_name.is_empty() {
        let cleaned: String = p
            .short_name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect();
        if cleaned.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
            s = cleaned;
        }
    }
    s
}

fn audio_port(index: usize, symbol: &str, name: &str, input: bool) -> String {
    let dir = if input {
        "lv2:InputPort, lv2:AudioPort"
    } else {
        "lv2:OutputPort, lv2:AudioPort"
    };
    format!(
        r#"
    lv2:port [
        a {dir} ;
        lv2:index {index} ;
        lv2:symbol "{symbol}" ;
        lv2:name "{name}" ;
    ] ;"#
    )
}

fn range_bounds(range: ParamRange) -> (f64, f64) {
    match range {
        ParamRange::Linear { min, max }
        | ParamRange::Logarithmic { min, max }
        | ParamRange::Skewed { min, max, .. }
        | ParamRange::SymmetricalSkewed { min, max, .. } => (min, max),
        ParamRange::Discrete { min, max } => (min as f64, max as f64),
        ParamRange::Enum { count } => (0.0, count.saturating_sub(1) as f64),
        ParamRange::Reversed(inner) => range_bounds(*inner),
    }
}

fn control_port(index: usize, symbol: &str, p: &ParamInfo) -> String {
    let (min, max) = range_bounds(p.range);
    let def = p.default_plain;

    let mut props = String::new();
    if matches!(
        p.kind,
        ParamValueKind::Int | ParamValueKind::Bool | ParamValueKind::Enum
    ) {
        props.push_str("\n        lv2:portProperty lv2:integer ;");
    }
    if p.kind == ParamValueKind::Bool {
        props.push_str("\n        lv2:portProperty lv2:toggled ;");
    }

    format!(
        r#"
    lv2:port [
        a lv2:InputPort, lv2:ControlPort ;
        lv2:index {index} ;
        lv2:symbol "{symbol}" ;
        lv2:name "{name}" ;
        lv2:default {def} ;
        lv2:minimum {min} ;
        lv2:maximum {max} ;{props}
    ] ;"#,
        index = index,
        symbol = symbol,
        name = escape_ttl(p.name),
        def = def,
        min = min,
        max = max,
        props = props,
    )
}

fn midi_in_port(index: usize) -> String {
    format!(
        r#"
    lv2:port [
        a lv2:InputPort, lv2:AtomPort ;
        lv2:index {index} ;
        lv2:symbol "midi_in" ;
        lv2:name "MIDI In" ;
        atom:bufferType atom:Sequence ;
        atom:supports midi:MidiEvent ;
    ] ;"#
    )
}

fn escape_ttl(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use aura_core::PluginInfo;

    fn one_param() -> Vec<ParamInfo> {
        vec![ParamInfo {
            id: 1,
            name: "Gain",
            short_name: "gain",
            group: "",
            range: ParamRange::Linear {
                min: -24.0,
                max: 24.0,
            },
            default_plain: 0.0,
            flags: aura_params::ParamFlags::empty(),
            unit: aura_params::ParamUnit::None,
            kind: ParamValueKind::Float,
            midi_map: None,
            midi_channel: None,
        }]
    }

    #[test]
    fn ttl_mentions_uri_and_ports() {
        let info = PluginInfo::new("Test", "LX", "0.1.0", "test");
        let ttl = generate_ttl(&info, "https://example.com/lv2/test", "test_plug", &one_param());
        assert!(ttl.manifest.contains("https://example.com/lv2/test"));
        assert!(ttl.plugin.contains("lv2:AudioPort"));
        assert!(ttl.plugin.contains("in_l"));
        assert!(ttl.plugin.contains("p_1") || ttl.plugin.contains("gain"));
        assert!(ttl.plugin.contains("lv2:ControlPort"));
    }

    #[test]
    fn mono_ttl_has_single_in_out() {
        let info = PluginInfo::new("Test", "LX", "0.1.0", "test");
        let ttl = generate_ttl_with_layout(
            &info,
            "https://example.com/lv2/mono",
            "mono_plug",
            &one_param(),
            BusLayout::mono(),
        );
        assert!(ttl.plugin.contains("lv2:symbol \"in\""));
        assert!(ttl.plugin.contains("lv2:symbol \"out\""));
        assert!(!ttl.plugin.contains("in_l"));
        // control index 2 after mono in/out
        assert!(ttl.plugin.contains("lv2:index 2"));
    }

    #[test]
    fn midi_port_appended_last_when_accepted() {
        let mut info = PluginInfo::new("Test", "LX", "0.1.0", "test");
        info.accepts_midi_in = true;
        // stereo: 0/1 in, 2/3 out, 4 control → midi at 5
        let ttl = generate_ttl(&info, "https://example.com/lv2/test", "test_plug", &one_param());
        assert!(ttl.plugin.contains("lv2:AtomPort"));
        assert!(ttl.plugin.contains("atom:bufferType atom:Sequence"));
        assert!(ttl.plugin.contains("atom:supports midi:MidiEvent"));
        assert!(ttl.plugin.contains("lv2:index 5"));
        assert!(ttl.plugin.contains("lv2:symbol \"midi_in\""));

        info.accepts_midi_in = false;
        let ttl = generate_ttl(&info, "https://example.com/lv2/test", "test_plug", &one_param());
        assert!(!ttl.plugin.contains("lv2:AtomPort"));
    }
}
