//! Shared scaffold engine for `cargo aura new` / `init` / `add`.
//!
//! Pure template emission: [`files`] returns `(relative path, contents)`
//! pairs; the commands own every filesystem decision (destination dir,
//! overwrite rules, cleanup on failure).

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

/// Plugin kind template (`--kind`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Stereo main I/O gain FX (default).
    Effect,
    /// Mono main I/O gain FX (`BusLayout::mono`).
    EffectMono,
    /// Stereo pass-through with level meters (no FFT — product keeps analysis DSP).
    Analyzer,
}

impl Kind {
    pub const SUPPORTED: &'static str = "effect, effect-mono, analyzer";

    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "effect" => Ok(Self::Effect),
            "effect-mono" => Ok(Self::EffectMono),
            "analyzer" => Ok(Self::Analyzer),
            other => Err(format!(
                "unknown kind '{other}' (supported: {})",
                Self::SUPPORTED
            )),
        }
    }

    /// `aura.toml` category string for the scaffolded plugin.
    pub fn category(self) -> &'static str {
        match self {
            Self::Effect | Self::EffectMono => "effect",
            Self::Analyzer => "analyzer",
        }
    }

    /// Rust `PluginCategory::…` token for `PluginLogic::info`.
    fn plugin_category_rs(self) -> &'static str {
        match self {
            Self::Effect | Self::EffectMono => "PluginCategory::Effect",
            Self::Analyzer => "PluginCategory::Analyzer",
        }
    }

    /// Optional `bus_layouts()` body inserted into `PluginLogic`.
    fn bus_layouts_fn(self) -> &'static str {
        match self {
            Self::EffectMono => {
                r"
    fn bus_layouts() -> Vec<BusLayout> {
        vec![BusLayout::mono()]
    }
"
            }
            Self::Effect | Self::Analyzer => "",
        }
    }

    fn kind_doc(self) -> &'static str {
        match self {
            Self::Effect => "stereo effect",
            Self::EffectMono => "mono effect",
            Self::Analyzer => "analyzer (meter stub)",
        }
    }
}

/// Everything the templates need.
pub struct ScaffoldSpec {
    /// Package name (kebab/snake), e.g. `smoke-gain`.
    pub name: String,
    /// Extra formats beyond the always-on CLAP (`vst3`, `lv2`).
    pub formats: Vec<String>,
    /// Path-dep root for AURA (forward slashes, no verbatim prefix).
    pub aura_root: String,
    pub kind: Kind,
}

impl ScaffoldSpec {
    pub fn crate_name(&self) -> String {
        self.name.replace('-', "_")
    }

    /// Human display name: `smoke-gain` → `Smoke Gain`.
    pub fn display(&self) -> String {
        title_case(&self.name)
    }

    fn struct_name(&self) -> String {
        to_struct_name(&self.crate_name())
    }
}

/// All scaffold files as `(relative path, contents)`. Pure — no I/O.
///
/// Emission is long but linear — splitting it would only obscure it.
#[allow(clippy::too_many_lines)]
pub fn files(spec: &ScaffoldSpec) -> Vec<(String, String)> {
    let name = &spec.name;
    let display = spec.display();
    let struct_name = spec.struct_name();
    let params_name = format!("{struct_name}Params");

    // Format features beyond the always-on CLAP.
    let mut feature_lines = String::from("default = [\"clap\"]\nclap = [\"aura/clap\"]\n");
    let mut human: Vec<&str> = vec!["CLAP"];
    let mut flags = String::from("--clap");
    for f in &spec.formats {
        let _ = writeln!(feature_lines, "{f} = [\"aura/{f}\"]");
        human.push(match f.as_str() {
            "vst3" => "VST3",
            "lv2" => "LV2",
            other => other,
        });
        let _ = write!(flags, " --{f}");
    }
    let formats_doc = human.join(" + ");

    let cargo_toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2024"
license = "GPL-3.0-or-later"
description = "{display} — AURA plugin"
publish = false

# Standalone package (not a member of the AURA framework workspace).
[workspace]

[lib]
crate-type = ["cdylib", "lib"]

[features]
{feature_lines}
[dependencies]
aura = {{ path = "{}/crates/aura" }}
aura-editor = {{ path = "{}/crates/aura-editor", features = ["backend-femtovg"] }}
slint = {{ version = "=1.17.1", default-features = false, features = ["std", "compat-1-2"] }}

[build-dependencies]
aura-build = {{ path = "{}/crates/aura-build" }}
"#,
        spec.aura_root, spec.aura_root, spec.aura_root
    );

    let build_rs = r#"fn main() {
    aura_build::compile("ui/main.slint").expect("slint compile");
}
"#
    .to_string();

    let category = spec.kind.category();
    let aura_toml = format!(
        r#"[vendor]
name = "LX Audiolabs"
id = "lx"
url = "https://lx-audiolabs.com"

[[plugin]]
name = "{display}"
bundle_id = "{name}"
crate = "{name}"
category = "{category}"

# Where `cargo aura install --clap|--vst3|--lv2` copies artifacts.
# `dir` is the base; format subdirs CLAP / VST3 / LV2 are appended.
# Env vars expand: %LOCALAPPDATA%, %COMMONPROGRAMFILES%, $HOME, …
# Per-format full path: clap = "C:\\Program Files\\Common Files\\CLAP"
# Env CLAPINS / VST3INS still override when set.
[install]
dir = "%LOCALAPPDATA%\\Programs\\Common"
"#
    );

    let agal_toml = format!(
        r#"# agal orientation for this plugin workspace
# https://github.com/LX-Audiolabs/agal

[project]
name = "{name}"
"#
    );

    let gitignore = "/target\n*.clap\n*.vst3\n*.lv2\n.DS_Store\n".to_string();

    let main_slint = match spec.kind {
        Kind::Effect | Kind::EffectMono => effect_main_slint(&display),
        Kind::Analyzer => analyzer_main_slint(&display),
    };

    let mut exports = format!("#[cfg(feature = \"clap\")]\naura::export!({struct_name});");
    for f in &spec.formats {
        let _ = write!(
            exports,
            "\n\n#[cfg(feature = \"{f}\")]\naura::export_{f}!({struct_name});"
        );
    }

    let lib_rs = match spec.kind {
        Kind::Effect | Kind::EffectMono => effect_lib_rs(
            &display,
            name,
            &struct_name,
            &params_name,
            &formats_doc,
            &flags,
            &exports,
            spec.kind,
        ),
        Kind::Analyzer => analyzer_lib_rs(
            &display,
            name,
            &struct_name,
            &params_name,
            &formats_doc,
            &flags,
            &exports,
            spec.kind,
        ),
    };

    vec![
        ("Cargo.toml".to_string(), cargo_toml),
        ("build.rs".to_string(), build_rs),
        ("aura.toml".to_string(), aura_toml),
        ("agal.toml".to_string(), agal_toml),
        (".gitignore".to_string(), gitignore),
        ("ui/main.slint".to_string(), main_slint),
        ("src/lib.rs".to_string(), lib_rs),
    ]
}

fn effect_main_slint(display: &str) -> String {
    format!(
        r#"// {display} — AURA + Slint (Material 3–aligned @aura tokens)
import {{ Knob, AuraTheme }} from "@aura";

// AURA standard fonts (bundled via aura-build): import registers them
// compile-time, default-font-family makes text identical across OSes.
import "NotoSans-Regular.ttf";
import "NotoSans-Bold.ttf";

export component AppWindow inherits Window {{
    preferred-width: 320px;
    preferred-height: 220px;
    background: AuraTheme.surface;
    default-font-family: "Noto Sans";

    in-out property <float> gain: 0.0;
    callback gain-changed(float);

    VerticalLayout {{
        padding: 16px;
        spacing: 12px;

        Rectangle {{
            background: AuraTheme.surface-container;
            border-radius: AuraTheme.radius-md;
            border-width: 1px;
            border-color: AuraTheme.outline-variant;
            vertical-stretch: 1;

            VerticalLayout {{
                padding: 16px;
                spacing: 12px;
                alignment: center;

                Text {{
                    text: "{display}";
                    color: AuraTheme.on-surface;
                    font-size: AuraTheme.font-title;
                    font-weight: 600;
                    horizontal-alignment: center;
                }}

                HorizontalLayout {{
                    alignment: center;
                    Knob {{
                        label: "Gain";
                        minimum: -24.0;
                        maximum: 24.0;
                        value <=> root.gain;
                        value-text: round(root.gain * 10) / 10 + " dB";
                        changed(v) => {{ root.gain-changed(v); }}
                    }}
                }}
            }}
        }}
    }}
}}
"#
    )
}

fn analyzer_main_slint(display: &str) -> String {
    format!(
        r#"// {display} — AURA analyzer stub (meters only; FFT stays product-side)
import {{ Knob, Meter, AuraTheme }} from "@aura";

import "NotoSans-Regular.ttf";
import "NotoSans-Bold.ttf";

export component AppWindow inherits Window {{
    preferred-width: 360px;
    preferred-height: 240px;
    background: AuraTheme.surface;
    default-font-family: "Noto Sans";

    in-out property <float> trim: 0.0;
    in-out property <float> level-left: 0.0;
    in-out property <float> level-right: 0.0;
    callback trim-changed(float);

    VerticalLayout {{
        padding: 16px;
        spacing: 12px;

        Rectangle {{
            background: AuraTheme.surface-container;
            border-radius: AuraTheme.radius-md;
            border-width: 1px;
            border-color: AuraTheme.outline-variant;
            vertical-stretch: 1;

            VerticalLayout {{
                padding: 16px;
                spacing: 12px;
                alignment: center;

                Text {{
                    text: "{display}";
                    color: AuraTheme.on-surface;
                    font-size: AuraTheme.font-title;
                    font-weight: 600;
                    horizontal-alignment: center;
                }}

                HorizontalLayout {{
                    spacing: 20px;
                    alignment: center;

                    Knob {{
                        label: "Trim";
                        minimum: -24.0;
                        maximum: 24.0;
                        value <=> root.trim;
                        value-text: round(root.trim * 10) / 10 + " dB";
                        changed(v) => {{ root.trim-changed(v); }}
                    }}

                    Meter {{
                        level-left: root.level-left;
                        level-right: root.level-right;
                        preferred-height: 120px;
                    }}
                }}
            }}
        }}
    }}
}}
"#
    )
}

#[allow(clippy::too_many_arguments)]
fn effect_lib_rs(
    display: &str,
    name: &str,
    struct_name: &str,
    params_name: &str,
    formats_doc: &str,
    flags: &str,
    exports: &str,
    kind: Kind,
) -> String {
    let category = kind.plugin_category_rs();
    let bus = kind.bus_layouts_fn();
    let kind_doc = kind.kind_doc();
    format!(
        r#"//! {display} — AURA {kind_doc} ({formats_doc} via `aura-*` wrappers).
//!
//! ```bash
//! cargo aura build {flags} --release
//! cargo aura install {flags} --release
//! ```

use std::sync::Arc;

use aura::prelude::*;

slint::include_modules!();

// Generated by #[derive(Params)] — editor code never hardcodes raw IDs.
use {params_name}ParamId as P;

// ---------------------------------------------------------------------------
// Params
// ---------------------------------------------------------------------------

#[derive(Params)]
pub struct {params_name} {{
    // Every param pins an explicit `id = N` (wire-stable; never renumber).
    #[param(id = 1, name = "Gain", range = "linear(-24, 24)", default = 0.0, unit = "db")]
    pub gain: FloatParam,
}}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct {struct_name};

pub struct DspState;

impl PluginLogic for {struct_name} {{
    type Params = {params_name};
    type DspState = DspState;

    fn info() -> PluginInfo {{
        let mut info = PluginInfo::new(
            "{display}",
            "LX Audiolabs",
            env!("CARGO_PKG_VERSION"),
            "{name}",
        );
        info.clap_id = "com.lx-audiolabs.{name}";
        info.category = {category};
        info
    }}
{bus}
    fn init(_params: &Self::Params, _sample_rate: f64) -> Self::DspState {{
        DspState
    }}

    fn reset(_state: &mut Self::DspState, _params: &Self::Params, _config: &AudioConfig) {{}}

    fn process(
        _state: &mut Self::DspState,
        params: &Self::Params,
        buffer: &mut AudioBuffer<'_, f32>,
        _context: &mut ProcessContext,
    ) -> ProcessStatus {{
        let n = buffer.num_samples();
        #[allow(clippy::cast_possible_truncation)]
        let gain_db = params.gain.raw_target() as f32;
        let lin = 10.0f32.powf(gain_db / 20.0);

        // Channel-agnostic: works for mono and stereo layouts.
        let ch = buffer.num_outputs().min(buffer.num_inputs());
        for c in 0..ch {{
            // Copy input → output with gain (read first: in/out may alias).
            let input: Vec<f32> = buffer.input(c).to_vec();
            let out = buffer.output(c);
            for i in 0..n {{
                out[i] = input[i] * lin;
            }}
        }}
        // Extra outs: silence
        for c in ch..buffer.num_outputs() {{
            buffer.output(c).fill(0.0);
        }}
        ProcessStatus::Continue
    }}

    fn editor(_params: Arc<Self::Params>) -> Option<Box<dyn Editor>> {{
        Some(
            aura_editor::AuraSlintEditor::new(
                (320, 200),
                |ctx| {{
                    let ui = AppWindow::new().expect("slint component");
                    let params = ctx.params.clone();
                    ui.on_gain_changed(move |v| params.set_plain(P::Gain.id(), f64::from(v)));
                    ui
                }},
                |ui, ctx| {{
                    #[allow(clippy::cast_possible_truncation)]
                    let v = ctx.params.get_plain(P::Gain.id()).unwrap_or(0.0) as f32;
                    // Guard: don't fight an active drag with per-frame sync.
                    if (v - ui.get_gain()).abs() > 1.0e-4 {{
                        ui.set_gain(v);
                    }}
                }},
            )
            .into_editor(),
        )
    }}
}}

{exports}
"#
    )
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn analyzer_lib_rs(
    display: &str,
    name: &str,
    struct_name: &str,
    params_name: &str,
    formats_doc: &str,
    flags: &str,
    exports: &str,
    kind: Kind,
) -> String {
    let category = kind.plugin_category_rs();
    let kind_doc = kind.kind_doc();
    format!(
        r#"//! {display} — AURA {kind_doc} ({formats_doc} via `aura-*` wrappers).
//!
//! Peak meters only. FFT / spectrum DSP stays in product crates (`lx-analysis`).
//!
//! ```bash
//! cargo aura build {flags} --release
//! cargo aura install {flags} --release
//! ```

use std::sync::Arc;

use aura::prelude::*;

slint::include_modules!();

use {params_name}ParamId as P;

// ---------------------------------------------------------------------------
// Params
// ---------------------------------------------------------------------------

#[derive(Params)]
pub struct {params_name} {{
    #[param(id = 1, name = "Trim", range = "linear(-24, 24)", default = 0.0, unit = "db")]
    pub trim: FloatParam,
    // Peak levels written from process; host-readonly display params.
    #[param(id = 2, name = "Peak L", range = "linear(0, 1)", default = 0.0, flags = "readonly")]
    pub peak_l: FloatParam,
    #[param(id = 3, name = "Peak R", range = "linear(0, 1)", default = 0.0, flags = "readonly")]
    pub peak_r: FloatParam,
}}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct {struct_name};

pub struct DspState;

impl PluginLogic for {struct_name} {{
    type Params = {params_name};
    type DspState = DspState;

    fn info() -> PluginInfo {{
        let mut info = PluginInfo::new(
            "{display}",
            "LX Audiolabs",
            env!("CARGO_PKG_VERSION"),
            "{name}",
        );
        info.clap_id = "com.lx-audiolabs.{name}";
        info.category = {category};
        info
    }}

    fn init(_params: &Self::Params, _sample_rate: f64) -> Self::DspState {{
        DspState
    }}

    fn reset(_state: &mut Self::DspState, _params: &Self::Params, _config: &AudioConfig) {{}}

    fn process(
        _state: &mut Self::DspState,
        params: &Self::Params,
        buffer: &mut AudioBuffer<'_, f32>,
        _context: &mut ProcessContext,
    ) -> ProcessStatus {{
        let n = buffer.num_samples();
        #[allow(clippy::cast_possible_truncation)]
        let trim_db = params.trim.raw_target() as f32;
        let lin = 10.0f32.powf(trim_db / 20.0);

        let ch = buffer.num_outputs().min(buffer.num_inputs());
        let mut peak = [0.0f32; 2];
        for c in 0..ch {{
            let input: Vec<f32> = buffer.input(c).to_vec();
            let out = buffer.output(c);
            let mut ch_peak = 0.0f32;
            for i in 0..n {{
                let s = input[i] * lin;
                out[i] = s;
                ch_peak = ch_peak.max(s.abs());
            }}
            if c < 2 {{
                peak[c] = ch_peak;
            }}
        }}
        for c in ch..buffer.num_outputs() {{
            buffer.output(c).fill(0.0);
        }}
        // Mono: mirror peak to both meters.
        if ch == 1 {{
            peak[1] = peak[0];
        }}
        params.set_plain(P::PeakL.id(), f64::from(peak[0].clamp(0.0, 1.0)));
        params.set_plain(P::PeakR.id(), f64::from(peak[1].clamp(0.0, 1.0)));
        ProcessStatus::Continue
    }}

    fn editor(_params: Arc<Self::Params>) -> Option<Box<dyn Editor>> {{
        Some(
            aura_editor::AuraSlintEditor::new(
                (360, 220),
                |ctx| {{
                    let ui = AppWindow::new().expect("slint component");
                    let params = ctx.params.clone();
                    ui.on_trim_changed(move |v| params.set_plain(P::Trim.id(), f64::from(v)));
                    ui
                }},
                |ui, ctx| {{
                    #[allow(clippy::cast_possible_truncation)]
                    let trim = ctx.params.get_plain(P::Trim.id()).unwrap_or(0.0) as f32;
                    if (trim - ui.get_trim()).abs() > 1.0e-4 {{
                        ui.set_trim(trim);
                    }}
                    #[allow(clippy::cast_possible_truncation)]
                    let pl = ctx.params.get_plain(P::PeakL.id()).unwrap_or(0.0) as f32;
                    #[allow(clippy::cast_possible_truncation)]
                    let pr = ctx.params.get_plain(P::PeakR.id()).unwrap_or(0.0) as f32;
                    ui.set_level_left(pl);
                    ui.set_level_right(pr);
                }},
            )
            .into_editor(),
        )
    }}
}}

{exports}
"#
    )
}

/// Scaffold paths for `cargo aura add` (crate only — no root `aura.toml` / `agal.toml`).
pub fn plugin_crate_files(spec: &ScaffoldSpec) -> Vec<(String, String)> {
    files(spec)
        .into_iter()
        .filter(|(p, _)| p != "aura.toml" && p != "agal.toml")
        .collect()
}

// ---------------------------------------------------------------------------
// add-ui: shared Slint UI crate scaffold
// ---------------------------------------------------------------------------

/// Scaffold files for `cargo aura add-ui <name>` — a shared Slint component
/// library under `crates/<name>/`.
///
/// Emits a minimal theme + barrel; product widgets (knobs, meters, …) are
/// added by the author.
pub fn ui_crate_files(name: &str, aura_root: &str) -> Vec<(String, String)> {
    let display = title_case(name);

    let cargo_toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2024"
license = "GPL-3.0-or-later"
description = "{display} — shared Slint UI components"
publish = false

[dependencies]
slint = {{ version = "=1.17.1", default-features = false, features = ["std", "compat-1-2"] }}

[build-dependencies]
aura-build = {{ path = "{aura_root}/crates/aura-build" }}
"#
    );

    let build_rs = format!(
        r#"fn main() {{
    aura_build::compile("ui/{name}.slint").expect("slint compile");
}}
"#
    );

    let lib_rs = format!(
        r#"//! {display} — shared Slint UI components.
//!
//! Plugin crates import the `.slint` files directly:
//! ```text
//! import {{ {struct_name} }} from "../../../crates/{name}/ui/{name}.slint";
//! ```
//!
//! This crate compiles the barrel root so the module graph stays buildable
//! in isolation and can host future Rust helpers.

slint::include_modules!();
"#,
        struct_name = to_struct_name(&name.replace('-', "_"))
    );

    let barrel_slint = format!(
        r#"// {display} — shared Slint component barrel.
//
// Import from a plugin (path relative to plugin ui/main.slint):
//   import {{ {struct_name} }} from "../../../crates/{name}/ui/{name}.slint";
//
// Add your shared components below and re-export them here.

import "NotoSans-Regular.ttf";
import "NotoSans-Bold.ttf";

import {{ {struct_name} }} from "{name}-theme.slint";

export {{ {struct_name} }}
"#,
        struct_name = to_struct_name(&name.replace('-', "_"))
    );

    let theme_slint = format!(
        r#"// {display} — design tokens and theme.
//
// Override these values to match your brand. Plugin UIs reference them via
// `{struct_name}.surface`, `{struct_name}.radius-md`, etc.

export struct {struct_name} {{
    // ---- Palette ----
    out property <color> surface: #1E1E2E;
    out property <color> surface-container: #282840;
    out property <color> on-surface: #E0E0F0;
    out property <color> primary: #7C8AFF;
    out property <color> on-primary: #FFFFFF;
    out property <color> outline-variant: #3A3A50;

    // ---- Radius ----
    out property <length> radius-sm: 4px;
    out property <length> radius-md: 8px;

    // ---- Typography ----
    out property <length> font-title: 14px;
    out property <length> font-body: 12px;

    // ---- Spacing ----
    out property <length> spacing-xs: 4px;
    out property <length> spacing-sm: 8px;
    out property <length> spacing-md: 12px;
    out property <length> spacing-lg: 16px;
}}
"#,
        struct_name = to_struct_name(&name.replace('-', "_"))
    );

    vec![
        ("Cargo.toml".to_string(), cargo_toml),
        ("build.rs".to_string(), build_rs),
        ("src/lib.rs".to_string(), lib_rs),
        (format!("ui/{name}.slint"), barrel_slint),
        (format!("ui/{name}-theme.slint"), theme_slint),
    ]
}

/// `[[plugin]]` table to append to a workspace `aura.toml`.
pub fn plugin_table_block(display: &str, name: &str, category: &str, crate_path: &str) -> String {
    format!(
        r#"
[[plugin]]
name = "{display}"
bundle_id = "{name}"
crate = "{crate_path}"
category = "{category}"
"#
    )
}

/// Whether `aura.toml` text already lists `bundle_id = "…"`.
pub fn aura_toml_has_bundle(text: &str, bundle_id: &str) -> bool {
    let needle = format!("bundle_id = \"{bundle_id}\"");
    text.lines().any(|l| l.trim() == needle)
}

/// Append a `[[plugin]]` block; ensures a trailing newline before it.
pub fn append_plugin_table(text: &str, block: &str) -> String {
    let mut out = text.to_string();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    // Separate tables with a blank line when the file doesn't already end empty.
    if !out.ends_with("\n\n") {
        out.push('\n');
    }
    out.push_str(block.trim_start_matches('\n'));
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Write `files` under `dest`, creating parent dirs as needed.
pub fn write_files(dest: &Path, files: &[(String, String)]) -> io::Result<()> {
    for (rel, contents) in files {
        let path = dest.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, contents)?;
    }
    Ok(())
}

pub fn is_valid_crate_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn title_case(name: &str) -> String {
    name.split(['-', '_'])
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut c = s.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn to_struct_name(crate_name: &str) -> String {
    crate_name
        .split('_')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut c = s.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(name: &str, formats: &[&str]) -> ScaffoldSpec {
        ScaffoldSpec {
            name: name.to_string(),
            formats: formats.iter().map(|s| (*s).to_string()).collect(),
            aura_root: "L:/LX-Audiolabs/AURA".to_string(),
            kind: Kind::Effect,
        }
    }

    fn file<'a>(files: &'a [(String, String)], rel: &str) -> &'a str {
        files
            .iter()
            .find(|(p, _)| p == rel)
            .unwrap_or_else(|| panic!("missing scaffold file {rel}"))
            .1
            .as_str()
    }

    #[test]
    fn clap_only_file_set() {
        let files = files(&spec("smoke-gain", &[]));
        let paths: Vec<&str> = files.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(
            paths,
            [
                "Cargo.toml",
                "build.rs",
                "aura.toml",
                "agal.toml",
                ".gitignore",
                "ui/main.slint",
                "src/lib.rs",
            ]
        );
        let lib = file(&files, "src/lib.rs");
        assert!(lib.contains("aura::export!(SmokeGain);"), "{lib}");
        assert!(!lib.contains("export_vst3"), "{lib}");
        assert!(!lib.contains("export_lv2"), "{lib}");
    }

    #[test]
    fn multi_format_exports_and_features() {
        let files = files(&spec("my-plug", &["vst3", "lv2"]));
        let lib = file(&files, "src/lib.rs");
        assert!(lib.contains("aura::export_vst3!(MyPlug);"), "{lib}");
        assert!(lib.contains("aura::export_lv2!(MyPlug);"), "{lib}");
        let cargo = file(&files, "Cargo.toml");
        assert!(cargo.contains("vst3 = [\"aura/vst3\"]"), "{cargo}");
        assert!(cargo.contains("lv2 = [\"aura/lv2\"]"), "{cargo}");
        assert!(cargo.contains("name = \"my-plug\""), "{cargo}");
    }

    #[test]
    fn names_validate_and_convert() {
        assert!(is_valid_crate_name("smoke-gain"));
        assert!(is_valid_crate_name("aether_2"));
        assert!(!is_valid_crate_name("9lives"));
        assert!(!is_valid_crate_name("has space"));
        assert!(!is_valid_crate_name(""));

        let s = spec("smoke-gain", &[]);
        assert_eq!(s.crate_name(), "smoke_gain");
        assert_eq!(s.display(), "Smoke Gain");
        assert_eq!(s.struct_name(), "SmokeGain");
    }

    #[test]
    fn kind_parse() {
        assert_eq!(Kind::parse("effect"), Ok(Kind::Effect));
        assert_eq!(Kind::parse("effect-mono"), Ok(Kind::EffectMono));
        assert_eq!(Kind::parse("analyzer"), Ok(Kind::Analyzer));
        assert!(Kind::parse("instrument").is_err());
    }

    #[test]
    fn effect_mono_declares_mono_bus() {
        let mut s = spec("mono-gain", &[]);
        s.kind = Kind::EffectMono;
        let out = files(&s);
        let lib = file(&out, "src/lib.rs");
        let aura = file(&out, "aura.toml");
        assert!(lib.contains("BusLayout::mono()"), "{lib}");
        assert!(aura.contains("category = \"effect\""), "{aura}");
        // Process must not hardcode stereo.
        assert!(
            lib.contains("num_outputs().min(buffer.num_inputs())"),
            "{lib}"
        );
    }

    #[test]
    fn analyzer_template_meters_and_category() {
        let mut s = spec("peak-view", &[]);
        s.kind = Kind::Analyzer;
        let files = files(&s);
        let lib = file(&files, "src/lib.rs");
        let slint = file(&files, "ui/main.slint");
        let aura = file(&files, "aura.toml");
        assert!(lib.contains("PluginCategory::Analyzer"), "{lib}");
        assert!(lib.contains("PeakL"), "{lib}");
        assert!(lib.contains("set_level_left"), "{lib}");
        assert!(slint.contains("Meter"), "{slint}");
        assert!(aura.contains("category = \"analyzer\""), "{aura}");
        assert!(!lib.contains("BusLayout::mono()"), "{lib}");
    }

    #[test]
    fn plugin_crate_files_skip_root_manifests() {
        let out = plugin_crate_files(&spec("extra", &[]));
        let paths: Vec<&str> = out.iter().map(|(p, _)| p.as_str()).collect();
        assert!(!paths.contains(&"aura.toml"));
        assert!(!paths.contains(&"agal.toml"));
        assert!(paths.contains(&"src/lib.rs"));
        assert!(paths.contains(&"Cargo.toml"));
    }

    #[test]
    fn ui_crate_files_emits_minimal_scaffold() {
        let out = ui_crate_files("my-ui", "L:/LX-Audiolabs/AURA");
        let paths: Vec<&str> = out.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(
            paths,
            [
                "Cargo.toml",
                "build.rs",
                "src/lib.rs",
                "ui/my-ui.slint",
                "ui/my-ui-theme.slint",
            ]
        );
        let cargo = out
            .iter()
            .find(|(p, _)| p == "Cargo.toml")
            .unwrap()
            .1
            .as_str();
        assert!(cargo.contains("name = \"my-ui\""), "{cargo}");
        assert!(cargo.contains("aura-build"), "{cargo}");
        let barrel = out
            .iter()
            .find(|(p, _)| p == "ui/my-ui.slint")
            .unwrap()
            .1
            .as_str();
        assert!(
            barrel.contains("import { MyUi } from \"my-ui-theme.slint\""),
            "{barrel}"
        );
        assert!(barrel.contains("export { MyUi }"), "{barrel}");
        let theme = out
            .iter()
            .find(|(p, _)| p == "ui/my-ui-theme.slint")
            .unwrap()
            .1
            .as_str();
        assert!(theme.contains("export struct MyUi"), "{theme}");
        assert!(theme.contains("property <color> surface:"), "{theme}");
    }

    #[test]
    fn append_plugin_detects_bundle_and_appends() {
        let base = r#"[vendor]
name = "LX"

[[plugin]]
name = "First"
bundle_id = "first"
crate = "first"
category = "effect"
"#;
        assert!(aura_toml_has_bundle(base, "first"));
        assert!(!aura_toml_has_bundle(base, "second"));
        let block = plugin_table_block("Second", "second", "analyzer", "plugins/second");
        let merged = append_plugin_table(base, &block);
        assert!(merged.contains("bundle_id = \"second\""));
        assert!(merged.contains("crate = \"plugins/second\""));
        assert!(merged.contains("category = \"analyzer\""));
        // Original plugin still present.
        assert!(merged.contains("bundle_id = \"first\""));
    }
}
