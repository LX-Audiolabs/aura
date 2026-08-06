//! cargo-aura — build tool for AURA audio plugins.
//!
//! Install (from AURA repo):
//!   cargo install --path tools/cargo-aura --force
//!
//! Usage:
//!   cargo aura new my-plugin
//!   cargo aura build [--clap|--vst3|--lv2]
//!   cargo aura install [--clap|--vst3|--lv2]
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

    let cmd = args.first().map(String::as_str).unwrap_or("help");
    match cmd {
        "new" => cmd_new(&args[1..]),
        "build" => cmd_build(&args[1..]),
        "install" => cmd_install(&args[1..]),
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
  new <name>              Scaffold a plugin project (Slint + aura.toml + agal)
  build [--clap|--vst3|--lv2] [--release]
                          cargo build with format feature(s)
  install [--clap|--vst3|--lv2] [--release]
                          build + copy artifact into host search path
  doctor                  Check toolchain / AURA path / clap-validator
  help                    This message

Environment:
  AURA_PATH               Path to the AURA framework root (crates/, tools/)
  CLAPINS / CLAP_PATH     CLAP install directory (install --clap)

Status: early — CLAP wrapper not shipped yet; scaffold + build wiring work.
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
        println!("  --  clap-validator not on PATH (optional until CLAP ships)");
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
            println!("  !!  {bin} not found");
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
    println!("  cargo check");
    println!("  cargo aura build --clap   # when aura-clap ships");
    ExitCode::SUCCESS
}

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
# Format features will wire to aura/clap etc. when wrappers land.
clap = []
vst3 = []
lv2 = []

[dependencies]
aura = {{ path = "{aura_root}/crates/aura" }}
aura-baseview = {{ path = "{aura_root}/crates/aura-baseview", features = ["backend-femtovg"] }}
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
            r#"// {display} — AURA + Slint
import {{ Knob }} from "@aura";
import "NotoSans-Regular.ttf";

export component AppWindow inherits Window {{
    title: "{display}";
    preferred-width: 320px;
    preferred-height: 200px;
    background: #1a1a1e;

    VerticalLayout {{
        padding: 16px;
        spacing: 12px;

        Text {{
            text: "{display}";
            color: #e8e8ec;
            font-size: 18px;
            font-family: "Noto Sans";
            horizontal-alignment: center;
        }}

        Knob {{
            label: "Gain";
            minimum: -24;
            maximum: 24;
            value: 0;
            horizontal-stretch: 1;
            vertical-stretch: 1;
        }}
    }}
}}
"#
        ),
    )?;

    fs::write(
        dest.join("src/lib.rs"),
        format!(
            r#"//! {display} — AURA plugin scaffold.
//!
//! `PluginLogic` + CLAP export land as the framework grows (`aura-clap`).
//! This crate already compiles against `aura`, `aura-editor`, and `aura-build`.

use aura::prelude::*;

/// Plugin metadata (used once format wrappers register the plugin).
pub fn plugin_info() -> PluginInfo {{
    PluginInfo::new(
        "{display}",
        "LX Audiolabs",
        env!("CARGO_PKG_VERSION"),
        "{name}",
    )
}}

// ---------------------------------------------------------------------------
// DSP stub — wire to aura_core::PluginLogic when you add real processing.
// Params derive arrives with aura-derive; hand-written Params until then.
// ---------------------------------------------------------------------------

/// Placeholder so authors see the intended surface.
pub struct {struct_name};

impl {struct_name} {{
    pub fn info() -> PluginInfo {{
        plugin_info()
    }}
}}

// Keep the name for later: `impl PluginLogic for {struct_name} {{ ... }}`
const _: fn() = || {{
    let _ = {struct_name}::info;
}};

// Force-link aura + keep unused import lint quiet in empty scaffolds.
#[allow(dead_code)]
fn _aura_surface() {{
    let _ = ProcessMode::Realtime;
    let _ = ParamUnit::Db;
}}
"#,
            display = display,
            name = name,
            struct_name = to_struct_name(crate_name),
        ),
    )?;

    Ok(())
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
            if matches!(f.as_str(), "clap" | "vst3" | "lv2") {
                eprintln!(
                    "note: feature `{f}` is declared in scaffolds; format wrapper crates not shipped yet"
                );
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
        Ok(s) => ExitCode::from(s.code().unwrap_or(1) as u8),
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
            "vst3" | "lv2" => {
                eprintln!("install --{feat}: not implemented yet (format wrapper pending)");
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

    let candidates = find_plugin_artifacts(&dir)?;
    if candidates.is_empty() {
        return Err(format!(
            "no cdylib/artifact in {} — build a plugin package first (crate-type cdylib)",
            dir.display()
        ));
    }

    let dest_root = clap_install_dir()?;
    fs::create_dir_all(&dest_root).map_err(|e| e.to_string())?;

    for src in candidates {
        let file_name = src
            .file_name()
            .ok_or_else(|| "bad path".to_string())?
            .to_string_lossy()
            .into_owned();
        // Normalize to .clap on copy when we only have .dll/.so/.dylib
        let dest_name = if file_name.ends_with(".clap") {
            file_name
        } else if let Some(stem) = strip_lib_prefix(&file_name) {
            format!("{stem}.clap")
        } else {
            format!(
                "{}.clap",
                Path::new(&file_name)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
            )
        };
        let dest = dest_root.join(&dest_name);
        fs::copy(&src, &dest).map_err(|e| format!("copy {} → {}: {e}", src.display(), dest.display()))?;
        println!("installed {}", dest.display());
        eprintln!("note: without aura-clap the binary may not be a valid CLAP yet");
    }
    Ok(())
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
        // Skip deps, build scripts, cargo-aura itself, rlibs
        if name.ends_with(".d") || name.ends_with(".rlib") || name.ends_with(".rmeta") {
            continue;
        }
        if name.ends_with(".clap")
            || name.ends_with(".dll")
            || name.ends_with(".so")
            || name.ends_with(".dylib")
        {
            // Heuristic: skip known non-plugin names
            let lower = name.to_ascii_lowercase();
            if lower.contains("cargo_aura") || lower.starts_with("std-") {
                continue;
            }
            out.push(p);
        }
    }
    // Prefer package lib over random deps: usually few at top level of target/debug
    out.sort();
    Ok(out)
}

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
    env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target"))
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

fn strip_lib_prefix(name: &str) -> Option<&str> {
    // libfoo.so / libfoo.dylib → foo
    let n = name
        .strip_suffix(".so")
        .or_else(|| name.strip_suffix(".dylib"))
        .or_else(|| name.strip_suffix(".dll"))?;
    Some(n.strip_prefix("lib").unwrap_or(n))
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
