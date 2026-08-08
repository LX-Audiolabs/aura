//! Shared scaffold engine for `cargo aura new` / `cargo aura init`.
//!
//! Pure template emission: [`files`] returns `(relative path, contents)`
//! pairs; the commands own every filesystem decision (destination dir,
//! overwrite rules, cleanup on failure).

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

/// Plugin kind template. Only `effect` ships today; the flag surface exists
/// so `new`/`init` stay stable when analyzer/instrument templates land.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Effect,
}

impl Kind {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "effect" => Ok(Self::Effect),
            other => Err(format!("unknown kind '{other}' (supported: effect)")),
        }
    }

    /// `aura.toml` category string for the scaffolded plugin.
    pub fn category(self) -> &'static str {
        match self {
            Self::Effect => "effect",
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
# Workaround: zune-core 0.5.2 ships empty log macros that break zune-jpeg 0.5.15
# (pulled via slint-build). Pin until fixed upstream, then delete this line.
zune-core = "=0.5.1"

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

    let main_slint = format!(
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
    );

    let mut exports = format!("#[cfg(feature = \"clap\")]\naura::export!({struct_name});");
    for f in &spec.formats {
        let _ = write!(
            exports,
            "\n\n#[cfg(feature = \"{f}\")]\naura::export_{f}!({struct_name});"
        );
    }

    let lib_rs = format!(
        r#"//! {display} — AURA plugin ({formats_doc} via `aura-*` wrappers).
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
        info.category = PluginCategory::Effect;
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
        let gain_db = params.gain.raw_target() as f32;
        let lin = 10.0f32.powf(gain_db / 20.0);

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
    );

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
        assert!(Kind::parse("instrument").is_err());
    }
}
