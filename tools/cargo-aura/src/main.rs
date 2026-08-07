//! cargo-aura — build tool for AURA audio plugins.
//!
//! Install (from AURA repo):
//!   cargo install --path tools/cargo-aura --force
//!
//! Usage:
//!   cargo aura new my-plugin
//!   cargo aura build [--clap|--vst3|--lv2]
//!   cargo aura install [--clap|--vst3|--lv2]
//!   cargo aura preview [path] [--no-watch]
//!   cargo aura doctor

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    // Cargo invokes us as `cargo-aura aura <args…>` or `cargo-aura <args…>`.
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("aura") {
        args.remove(0);
    }

    let cmd = args.first().map_or("help", String::as_str);
    match cmd {
        "new" => cmd_new(&args[1..]),
        "build" => cmd_build(&args[1..]),
        "install" => cmd_install(&args[1..]),
        "preview" => cmd_preview(&args[1..]),
        "doctor" => cmd_doctor(),
        "help" | "--help" | "-h" => {
            print_help();
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("unknown command: {other}");
            print_help();
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    eprintln!(
        "\
cargo-aura — build tool for AURA audio plugins

Usage:
  cargo aura <command> [options]

Commands:
  new <name>              Scaffold a plugin project (Slint + derive + aura.toml + agal)
  build [--clap|--vst3|--lv2] [--release]
                          cargo build with format feature(s)
  install [--clap|--vst3|--lv2] [--release]
                          build + copy artifact into host search path
  preview [path] [--component N] [--no-watch]
                          hot-reload the plugin .slint UI (default ui/main.slint)
  doctor                  Check toolchain / AURA path / clap-validator
  help                    This message

Environment:
  AURA_PATH               Path to the AURA framework root (crates/, tools/)
  CLAPINS / CLAP_PATH     CLAP install directory (install --clap)
  VST3INS / VST3_PATH     VST3 install directory (install --vst3)

Status: CLAP + VST3 path ship (scaffold/build/install + parented GUI); LV2 pending.
"
    );
}

// ---------------------------------------------------------------------------
// doctor
// ---------------------------------------------------------------------------

fn cmd_doctor() -> ExitCode {
    let mut ok = true;
    println!("cargo-aura doctor\n");

    ok &= check_cmd("rustc", &["--version"]);
    ok &= check_cmd("cargo", &["--version"]);

    match aura_root() {
        Ok(root) => {
            println!("  ok  AURA_PATH  {}", root.display());
            for need in [
                "crates/aura",
                "crates/aura-core",
                "crates/aura-baseview",
                "crates/aura-editor",
                "crates/aura-build",
            ]
            {
                let p = root.join(need);
                if p.is_dir() || p.join("Cargo.toml").is_file() {
                    println!("  ok  {need}");
                } else {
                    println!("  !!  missing {need}");
                    ok = false;
                }
            }
        }
        Err(e) => {
            println!("  !!  AURA_PATH  {e}");
            ok = false;
        }
    }

    if which("clap-validator").is_some() {
        println!("  ok  clap-validator on PATH");
    } else {
        println!("  --  clap-validator not on PATH (optional, recommended)");
    }

    if ok {
        println!("\ndoctor: lookin' good.");
        ExitCode::SUCCESS
    } else {
        println!("\ndoctor: fix the !! items.");
        ExitCode::FAILURE
    }
}

fn check_cmd(bin: &str, args: &[&str]) -> bool {
    match Command::new(bin).args(args).output() {
        Ok(o) if o.status.success() => {
            let line = String::from_utf8_lossy(&o.stdout);
            let first = line.lines().next().unwrap_or("").trim();
            println!("  ok  {bin:12} {first}");
            true
        }
        _ => {
            eprintln!("  !!  {bin} not found");
            false
        }
    }
}

// ---------------------------------------------------------------------------
// new
// ---------------------------------------------------------------------------

fn cmd_new(args: &[String]) -> ExitCode {
    let name = match args.first() {
        Some(n) if !n.starts_with('-') => n.as_str(),
        _ => {
            eprintln!("usage: cargo aura new <name>");
            return ExitCode::FAILURE;
        }
    };

    if !is_valid_crate_name(name) {
        eprintln!("error: '{name}' is not a valid Cargo package name (use snake_case / kebab-case letters)");
        return ExitCode::FAILURE;
    }

    let dest = PathBuf::from(name);
    if dest.exists() {
        eprintln!("error: {} already exists", dest.display());
        return ExitCode::FAILURE;
    }

    let root = match aura_root() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    // Prefer path deps with forward slashes for Cargo.toml portability.
    // Windows canonicalize() may yield `\\?\C:\...` — Cargo path deps reject that.
    let root_s = strip_verbatim_prefix(&root)
        .to_string_lossy()
        .replace('\\', "/");
    let crate_name = name.replace('-', "_");
    let display = title_case(name);

    if let Err(e) = write_scaffold(&dest, name, &crate_name, &display, &root_s) {
        eprintln!("error: scaffold failed: {e}");
        let _ = fs::remove_dir_all(&dest);
        return ExitCode::FAILURE;
    }

    println!("created {}", dest.display());
    println!("  aura.toml  agal.toml  ui/main.slint  src/lib.rs");
    println!();
    println!("next:");
    println!("  cd {name}");
    println!("  cargo aura build --clap");
    println!("  cargo aura install --clap --release   # into host CLAP path");
    ExitCode::SUCCESS
}

// Template emission is long but linear — splitting it would only obscure it.
#[allow(clippy::too_many_lines)]
fn write_scaffold(
    dest: &Path,
    name: &str,
    crate_name: &str,
    display: &str,
    aura_root: &str,
) -> std::io::Result<()> {
    fs::create_dir_all(dest.join("src"))?;
    fs::create_dir_all(dest.join("ui"))?;

    fs::write(
        dest.join("Cargo.toml"),
        format!(
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
default = ["clap"]
clap = ["aura/clap"]

[dependencies]
aura = {{ path = "{aura_root}/crates/aura" }}
aura-editor = {{ path = "{aura_root}/crates/aura-editor", features = ["backend-femtovg"] }}
slint = {{ version = "=1.17.1", default-features = false, features = ["std", "compat-1-2"] }}

[build-dependencies]
aura-build = {{ path = "{aura_root}/crates/aura-build" }}
"#
        ),
    )?;

    fs::write(
        dest.join("build.rs"),
        r#"fn main() {
    aura_build::compile("ui/main.slint").expect("slint compile");
}
"#,
    )?;

    fs::write(
        dest.join("aura.toml"),
        format!(
            r#"[vendor]
name = "LX Audiolabs"
id = "lx"
url = "https://lx-audiolabs.com"

[[plugin]]
name = "{display}"
bundle_id = "{name}"
crate = "{name}"
category = "effect"
"#
        ),
    )?;

    fs::write(
        dest.join("agal.toml"),
        format!(
            r#"# agal orientation for this plugin workspace
# https://github.com/LX-Audiolabs/agal

[project]
name = "{name}"
"#
        ),
    )?;

    fs::write(
        dest.join(".gitignore"),
        "/target\n*.clap\n*.vst3\n*.lv2\n.DS_Store\n",
    )?;

    fs::write(
        dest.join("ui/main.slint"),
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
        ),
    )?;

    fs::write(
        dest.join("src/lib.rs"),
        format!(
            r#"//! {display} — AURA plugin (CLAP via `aura-clap`).
//!
//! ```bash
//! cargo aura build --clap --release
//! cargo aura install --clap --release
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

#[cfg(feature = "clap")]
aura::export!({struct_name});
"#,
            params_name = format!("{struct_name}Params", struct_name = to_struct_name(crate_name)),
            struct_name = to_struct_name(crate_name),
        ),
    )?;

    Ok(())
}

// ---------------------------------------------------------------------------
// preview
// ---------------------------------------------------------------------------

/// `cargo aura preview [path] [--component N] [--no-watch]` — hot-reload the
/// plugin's `.slint` UI without compiling the plugin. Delegates to the
/// `aura-preview` binary in the AURA workspace.
fn cmd_preview(args: &[String]) -> ExitCode {
    let root = match aura_root() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    let mut cmd = Command::new("cargo");
    cmd.arg("run")
        .arg("-p")
        .arg("aura-preview")
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"))
        .arg("--")
        .args(args);

    match cmd.status() {
        Ok(s) if s.success() => ExitCode::SUCCESS,
        Ok(s) => ExitCode::from(u8::try_from(s.code().unwrap_or(1)).unwrap_or(1)),
        Err(e) => {
            eprintln!("failed to run cargo: {e}");
            ExitCode::FAILURE
        }
    }
}

// ---------------------------------------------------------------------------
// build / install
// ---------------------------------------------------------------------------

fn cmd_build(args: &[String]) -> ExitCode {
    let (features, release, rest) = parse_format_flags(args);
    if !rest.is_empty() {
        eprintln!("warning: ignoring extra args: {}", rest.join(" "));
    }

    if features.is_empty() {
        eprintln!("note: no --clap/--vst3/--lv2; building default features");
    } else {
        for f in &features {
            if f == "lv2" {
                eprintln!("note: feature `lv2` declared; format wrapper crate not shipped yet");
            }
        }
    }

    let mut cmd = Command::new("cargo");
    cmd.arg("build");
    if release {
        cmd.arg("--release");
    }
    if !features.is_empty() {
        cmd.arg("--features");
        cmd.arg(features.join(","));
    }

    match cmd.status() {
        Ok(s) if s.success() => ExitCode::SUCCESS,
        Ok(s) => ExitCode::from(u8::try_from(s.code().unwrap_or(1)).unwrap_or(1)),
        Err(e) => {
            eprintln!("failed to run cargo: {e}");
            ExitCode::FAILURE
        }
    }
}

fn cmd_install(args: &[String]) -> ExitCode {
    let (features, release, _) = parse_format_flags(args);
    if features.is_empty() {
        eprintln!("usage: cargo aura install --clap|--vst3|--lv2 [--release]");
        return ExitCode::FAILURE;
    }

    // Build first.
    let mut build_args: Vec<String> = features.iter().map(|f| format!("--{f}")).collect();
    if release {
        build_args.push("--release".into());
    }
    if cmd_build(&build_args) != ExitCode::SUCCESS {
        return ExitCode::FAILURE;
    }

    let profile = if release { "release" } else { "debug" };
    let target_dir = project_target_dir();

    for feat in &features {
        match feat.as_str() {
            "clap" => {
                if let Err(e) = install_clap(&target_dir, profile) {
                    eprintln!("install --clap: {e}");
                    return ExitCode::FAILURE;
                }
            }
            "vst3" => {
                if let Err(e) = install_vst3(&target_dir, profile) {
                    eprintln!("install --vst3: {e}");
                    return ExitCode::FAILURE;
                }
            }
            "lv2" => {
                eprintln!("install --lv2: not implemented yet (format wrapper pending)");
            }
            _ => {}
        }
    }
    ExitCode::SUCCESS
}

fn install_clap(target_dir: &Path, profile: &str) -> Result<(), String> {
    let dir = target_dir.join(profile);
    if !dir.is_dir() {
        return Err(format!("no build dir {}", dir.display()));
    }

    // Ship exactly this package's cdylib as `<package>.clap` — never a
    // random dependency artifact that happens to sit in the target dir.
    let pkg = package_name().ok_or("could not read [package] name from ./Cargo.toml")?;
    let crate_name = pkg.replace('-', "_");
    let candidates = find_plugin_artifacts(&dir)?;
    let src = candidates
        .iter()
        .find(|p| artifact_stem(p) == Some(crate_name.as_str()))
        .ok_or_else(|| {
            format!(
                "no built artifact for `{pkg}` in {} — crate-type cdylib + `cargo aura build --clap` first",
                dir.display()
            )
        })?;

    let dest_root = clap_install_dir()?;
    fs::create_dir_all(&dest_root).map_err(|e| e.to_string())?;

    let dest = dest_root.join(format!("{pkg}.clap"));
    fs::copy(src, &dest).map_err(|e| format!("copy {} → {}: {e}", src.display(), dest.display()))?;
    println!("installed {}", dest.display());
    Ok(())
}

/// Install as a VST3 module bundle:
/// ```text
/// <name>.vst3/Contents/<arch>/<name>.vst3   # renamed cdylib
/// ```
/// Arch folder follows Steinberg: `x86_64-win`, `x86_64-linux`, `MacOS`, …
fn install_vst3(target_dir: &Path, profile: &str) -> Result<(), String> {
    let dir = target_dir.join(profile);
    if !dir.is_dir() {
        return Err(format!("no build dir {}", dir.display()));
    }

    let pkg = package_name().ok_or("could not read [package] name from ./Cargo.toml")?;
    let crate_name = pkg.replace('-', "_");
    let candidates = find_plugin_artifacts(&dir)?;
    let src = candidates
        .iter()
        .find(|p| artifact_stem(p) == Some(crate_name.as_str()))
        .ok_or_else(|| {
            format!(
                "no built artifact for `{pkg}` in {} — crate-type cdylib + `cargo aura build --vst3` first",
                dir.display()
            )
        })?;

    let dest_root = vst3_install_dir()?;
    let bundle = dest_root.join(format!("{pkg}.vst3"));
    let arch_dir = bundle.join("Contents").join(vst3_arch_folder());
    fs::create_dir_all(&arch_dir).map_err(|e| e.to_string())?;

    // Binary inside the bundle uses the `.vst3` extension (still a PE/ELF/Mach-O).
    let dest = arch_dir.join(format!("{pkg}.vst3"));
    fs::copy(src, &dest).map_err(|e| format!("copy {} → {}: {e}", src.display(), dest.display()))?;
    println!("installed {}", bundle.display());
    Ok(())
}

fn vst3_arch_folder() -> &'static str {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "x86_64-win"
    }
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    {
        "arm64-win"
    }
    #[cfg(all(target_os = "windows", target_arch = "x86"))]
    {
        "x86-win"
    }
    #[cfg(target_os = "macos")]
    {
        // Universal / host-default layout hosts expect under Contents/MacOS.
        "MacOS"
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "x86_64-linux"
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        "aarch64-linux"
    }
    #[cfg(not(any(
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "x86"),
        target_os = "macos",
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
    )))]
    {
        "unknown-arch"
    }
}

#[allow(clippy::unnecessary_wraps)]
fn vst3_install_dir() -> Result<PathBuf, String> {
    if let Ok(p) = env::var("VST3INS").or_else(|_| env::var("VST3_PATH")) {
        return Ok(PathBuf::from(p));
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(cf) = env::var("COMMONPROGRAMFILES") {
            return Ok(PathBuf::from(cf).join("VST3"));
        }
        Ok(PathBuf::from(r"C:\Program Files\Common Files\VST3"))
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = env::var("HOME") {
            return Ok(PathBuf::from(home).join("Library/Audio/Plug-Ins/VST3"));
        }
        Err("HOME not set".into())
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(home) = env::var("HOME") {
            return Ok(PathBuf::from(home).join(".vst3"));
        }
        Err("HOME not set".into())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err("unsupported OS for default VST3 path; set VST3INS".into())
    }
}

/// `[package] name` from ./Cargo.toml (line scan — no toml dep needed).
fn package_name() -> Option<String> {
    let text = fs::read_to_string("Cargo.toml").ok()?;
    let mut in_package = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if in_package && line.starts_with("name") {
            let val = line.split('=').nth(1)?.trim();
            return Some(val.trim_matches('"').to_string());
        }
    }
    None
}

/// `libfoo.so` / `foo.dll` → `foo`.
fn artifact_stem(path: &Path) -> Option<&str> {
    let name = path.file_name()?.to_str()?;
    let stem = name.split('.').next()?;
    Some(stem.strip_prefix("lib").unwrap_or(stem))
}

fn find_plugin_artifacts(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    let rd = fs::read_dir(dir).map_err(|e| e.to_string())?;
    for ent in rd.flatten() {
        let p = ent.path();
        if !p.is_file() {
            continue;
        }
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        // Skip deps, build scripts, cargo-aura itself, rlibs
        if matches!(ext.as_deref(), Some("d" | "rlib" | "rmeta")) {
            continue;
        }
        if matches!(ext.as_deref(), Some("clap" | "dll" | "so" | "dylib")) {
            // Heuristic: skip known non-plugin names
            let lower = name.to_ascii_lowercase();
            if lower.contains("cargo_aura") || lower.starts_with("std-") {
                continue;
            }
            out.push(p);
        }
    }
    out.sort();
    Ok(out)
}

// Windows has no fallible path; macOS/Linux do — hence the Result.
#[allow(clippy::unnecessary_wraps)]
fn clap_install_dir() -> Result<PathBuf, String> {
    if let Ok(p) = env::var("CLAPINS").or_else(|_| env::var("CLAP_PATH")) {
        return Ok(PathBuf::from(p));
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(cf) = env::var("COMMONPROGRAMFILES") {
            return Ok(PathBuf::from(cf).join("CLAP"));
        }
        Ok(PathBuf::from(r"C:\Program Files\Common Files\CLAP"))
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = env::var("HOME") {
            return Ok(PathBuf::from(home).join("Library/Audio/Plug-Ins/CLAP"));
        }
        Err("HOME not set".into())
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(home) = env::var("HOME") {
            return Ok(PathBuf::from(home).join(".clap"));
        }
        Err("HOME not set".into())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err("unsupported OS for default CLAP path; set CLAPINS".into())
    }
}

fn project_target_dir() -> PathBuf {
    // Respect CARGO_TARGET_DIR; else ./target
    env::var_os("CARGO_TARGET_DIR").map_or_else(|| PathBuf::from("target"), PathBuf::from)
}

fn parse_format_flags(args: &[String]) -> (Vec<String>, bool, Vec<String>) {
    let mut features = Vec::new();
    let mut release = false;
    let mut rest = Vec::new();
    for a in args {
        match a.as_str() {
            "--clap" => features.push("clap".into()),
            "--vst3" => features.push("vst3".into()),
            "--lv2" => features.push("lv2".into()),
            "--release" => release = true,
            other => rest.push(other.to_string()),
        }
    }
    (features, release, rest)
}

// ---------------------------------------------------------------------------
// paths / utils
// ---------------------------------------------------------------------------

fn aura_root() -> Result<PathBuf, String> {
    if let Ok(p) = env::var("AURA_PATH") {
        let pb = PathBuf::from(p);
        if pb.join("crates").is_dir() {
            return pb.canonicalize().map_err(|e| e.to_string());
        }
        return Err(format!(
            "AURA_PATH={} has no crates/ directory",
            pb.display()
        ));
    }

    // tools/cargo-aura → AURA root
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cand = manifest.join("../..");
    if cand.join("crates/aura-core").exists() || cand.join("crates/aura").exists() {
        return cand.canonicalize().map_err(|e| e.to_string());
    }

    // Walk from cwd upward
    let mut dir = env::current_dir().map_err(|e| e.to_string())?;
    loop {
        if dir.join("crates/aura-core").exists() || dir.join("crates/aura").exists() {
            return Ok(dir);
        }
        if !dir.pop() {
            break;
        }
    }

    Err(
        "could not find AURA root — set AURA_PATH or run from inside the AURA repo".into(),
    )
}

fn which(bin: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let cand = dir.join(bin);
        if cand.is_file() {
            return Some(cand);
        }
        #[cfg(windows)]
        {
            let cand = dir.join(format!("{bin}.exe"));
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

fn is_valid_crate_name(name: &str) -> bool {
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

/// Drop Windows extended-length prefix (`\\?\`) so path deps work in Cargo.toml.
fn strip_verbatim_prefix(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path.to_path_buf()
    }
}
