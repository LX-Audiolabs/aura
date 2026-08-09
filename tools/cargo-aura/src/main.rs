//! cargo-aura — build tool for AURA audio plugins.
//!
//! Install (from AURA repo):
//!   cargo install --path tools/cargo-aura --force
//!
//! Usage:
//!   cargo aura new my-plugin
//!   cargo aura add other-plugin
//!   cargo aura build [--clap|--vst3|--lv2]
//!   cargo aura install [--clap|--vst3|--lv2]
//!   cargo aura preview [path] [--no-watch]
//!   cargo aura doctor

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

mod scaffold;

use scaffold::{Kind, ScaffoldSpec};

fn main() -> ExitCode {
    // Cargo invokes us as `cargo-aura aura <args…>` or `cargo-aura <args…>`.
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("aura") {
        args.remove(0);
    }

    let cmd = args.first().map_or("help", String::as_str);
    match cmd {
        "new" => cmd_new(&args[1..]),
        "init" => cmd_init(&args[1..]),
        "add" => cmd_add(&args[1..]),
        "add-ui" => cmd_add_ui(&args[1..]),
        "build" => cmd_build(&args[1..]),
        "install" => cmd_install(&args[1..]),
        "preview" => cmd_preview(&args[1..]),
        "gui" => cmd_gui(),
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
  new <name> [--vst3] [--lv2] [--kind <k>]
                          Scaffold a plugin project in ./<name>
                          (Slint + derive + aura.toml + agal)
                          CLAP is always on; flags add VST3 / LV2 feature + export
                          kinds: effect (default) | effect-mono | analyzer
  init [path] [--vst3] [--lv2] [--kind <k>]
                          Same scaffold, into an existing empty directory
                          (default: current dir; name comes from the dir name)
  add <name> [--vst3] [--lv2] [--kind <k>]
                          Add another plugin under plugins/<name>/ and append
                          [[plugin]] to the workspace aura.toml (re-open)
  add-ui <name>           Scaffold a shared Slint UI crate under crates/<name>/
                          (minimal theme + barrel; add your own components)
  build [--clap|--vst3|--lv2] [--release] [-plug <crate> [<crate>...]]
                          cargo build with format feature(s)
                          (-plug builds each selected workspace member in turn)
  install [--clap|--vst3|--lv2] [--release] [-plug <crate> [<crate>...]]
                          build + copy artifact into host search path
                          (-plug installs each selected plugin in turn)
  preview [path] [--component N] [--no-watch]
                          hot-reload the plugin .slint UI (default ui/main.slint)
  gui                     Open the visual project console (aura-gui)
  doctor                  Check toolchain / AURA path / clap-validator
  help                    This message

Install path (first match wins):
  1) env CLAPINS/CLAP_PATH or VST3INS/VST3_PATH
  2) aura.toml [install] clap= / vst3= / lv2=  (full path)
  3) aura.toml [install] dir=  + subdir CLAP|VST3|LV2
  4) OS host defaults (Program Files Common / ~/Library / ~/.clap)

  Paths expand %VAR% (Windows), $VAR / ${{VAR}}, and ~.
  Example: dir = \"%LOCALAPPDATA%\\Programs\\Common\"

Environment:
  AURA_PATH               Path to the AURA framework root (crates/, tools/)
  CLAPINS / CLAP_PATH     Override CLAP install directory
  VST3INS / VST3_PATH     Override VST3 install directory

Status: CLAP + VST3 + LV2 path ship (LV2: process/params/state + TTL bundle; no GUI yet).
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

    // agal is orientation only (agal_optional rule) — probe for info, never
    // a gate: builds/installs must work without it.
    if which("agal").is_some() {
        println!("  ok  agal on PATH (orientation mesh available)");
    } else {
        println!("  --  agal not on PATH (optional; orientation only, builds don't need it)");
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
// new / init (shared engine: scaffold.rs)
// ---------------------------------------------------------------------------

/// Parse `--vst3` / `--lv2` / `--clap` / `--kind <k>` (or `--kind=<k>`).
/// Returns extra formats (beyond the always-on CLAP), the kind, and the
/// positional args.
fn parse_scaffold_args(args: &[String]) -> Result<(Vec<String>, Kind, Vec<String>), String> {
    let mut formats: Vec<String> = Vec::new();
    let mut kind = Kind::Effect;
    let mut positional: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "--vst3" | "--lv2" => {
                let f = &a[2..];
                if !formats.iter().any(|x| x == f) {
                    formats.push(f.to_string());
                }
            }
            "--clap" => {} // default anyway
            "--kind" => {
                i += 1;
                let Some(v) = args.get(i) else {
                    return Err(format!(
                        "--kind needs a value (supported: {})",
                        Kind::SUPPORTED
                    ));
                };
                kind = Kind::parse(v)?;
            }
            _ if a.starts_with("--kind=") => kind = Kind::parse(&a["--kind=".len()..])?,
            _ if a.starts_with('-') => {
                return Err(format!("unknown flag '{a}' (want --vst3 / --lv2 / --kind)"));
            }
            _ => positional.push(a.clone()),
        }
        i += 1;
    }
    Ok((formats, kind, positional))
}

fn make_spec(name: &str, formats: Vec<String>, kind: Kind) -> Result<ScaffoldSpec, String> {
    if !scaffold::is_valid_crate_name(name) {
        return Err(format!(
            "'{name}' is not a valid Cargo package name (use snake_case / kebab-case letters)"
        ));
    }
    // Prefer path deps with forward slashes for Cargo.toml portability.
    // Windows canonicalize() may yield `\\?\C:\...` — Cargo path deps reject that.
    let aura_root = strip_verbatim_prefix(&aura_root()?)
        .to_string_lossy()
        .replace('\\', "/");
    Ok(ScaffoldSpec {
        name: name.to_string(),
        formats,
        aura_root,
        kind,
    })
}

fn print_scaffold_success(verb: &str, dest: &Path, cd: Option<&str>, formats: &[String]) {
    let mut all: Vec<&str> = vec!["clap"];
    all.extend(formats.iter().map(String::as_str));
    let flags = all
        .iter()
        .map(|f| format!("--{f}"))
        .collect::<Vec<_>>()
        .join(" ");

    println!("{verb} {}", dest.display());
    println!("  aura.toml  agal.toml  ui/main.slint  src/lib.rs");
    println!();
    println!("next:");
    if let Some(dir) = cd {
        println!("  cd {dir}");
    }
    println!("  cargo aura build {flags}");
    println!("  cargo aura install {flags} --release   # into host plugin paths");
}

fn cmd_new(args: &[String]) -> ExitCode {
    let (formats, kind, positional) = match parse_scaffold_args(args) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    if positional.len() > 1 {
        eprintln!("usage: cargo aura new <name> [--vst3] [--lv2] [--kind effect]");
        return ExitCode::FAILURE;
    }
    let Some(name) = positional.first() else {
        eprintln!("usage: cargo aura new <name> [--vst3] [--lv2] [--kind effect]");
        return ExitCode::FAILURE;
    };

    let dest = PathBuf::from(name);
    if dest.exists() {
        eprintln!("error: {} already exists", dest.display());
        return ExitCode::FAILURE;
    }

    let spec = match make_spec(name, formats, kind) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(e) = scaffold::write_files(&dest, &scaffold::files(&spec)) {
        eprintln!("error: scaffold failed: {e}");
        let _ = fs::remove_dir_all(&dest);
        return ExitCode::FAILURE;
    }

    print_scaffold_success("created", &dest, Some(name), &spec.formats);
    ExitCode::SUCCESS
}

/// `cargo aura init [path]` — same scaffold as `new`, but into an existing
/// empty directory (default: cwd). Package name comes from the dir name.
fn cmd_init(args: &[String]) -> ExitCode {
    let (formats, kind, positional) = match parse_scaffold_args(args) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    if positional.len() > 1 {
        eprintln!("usage: cargo aura init [path] [--vst3] [--lv2] [--kind effect]");
        return ExitCode::FAILURE;
    }

    let (dest, cd): (PathBuf, Option<String>) = match positional.first() {
        Some(p) => (PathBuf::from(p), Some(p.clone())),
        None => (PathBuf::from("."), None),
    };

    // Package name from the target dir (canonicalize resolves "." → dir name).
    let name_dir = if dest.exists() {
        match dest.canonicalize() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("error: cannot canonicalize {}: {e}", dest.display());
                return ExitCode::FAILURE;
            }
        }
    } else {
        dest.clone()
    };
    let Some(name) = name_dir.file_name().map(|s| s.to_string_lossy().into_owned()) else {
        eprintln!("error: cannot derive a package name from {}", dest.display());
        return ExitCode::FAILURE;
    };

    let spec = match make_spec(&name, formats, kind) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    // init never overwrites: every target path must be absent.
    let files = scaffold::files(&spec);
    for (rel, _) in &files {
        let p = dest.join(rel);
        if p.exists() {
            eprintln!(
                "error: {} already exists — init needs a directory without scaffold files",
                p.display()
            );
            return ExitCode::FAILURE;
        }
    }

    if let Err(e) = scaffold::write_files(&dest, &files) {
        eprintln!("error: scaffold failed: {e}");
        return ExitCode::FAILURE;
    }

    print_scaffold_success("initialized", &dest, cd.as_deref(), &spec.formats);
    ExitCode::SUCCESS
}

/// `cargo aura add <name>` — re-open: scaffold under `plugins/<name>/` and
/// append `[[plugin]]` to the workspace `aura.toml`.
fn cmd_add(args: &[String]) -> ExitCode {
    let (formats, kind, positional) = match parse_scaffold_args(args) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    if positional.len() != 1 {
        eprintln!(
            "usage: cargo aura add <name> [--vst3] [--lv2] [--kind {}]",
            Kind::SUPPORTED
        );
        return ExitCode::FAILURE;
    }
    let name = &positional[0];

    let aura_path = PathBuf::from("aura.toml");
    if !aura_path.is_file() {
        eprintln!(
            "error: no aura.toml in current directory — run `cargo aura new` / `init` first, \
             then `add` from that project root"
        );
        return ExitCode::FAILURE;
    }

    let aura_text = match fs::read_to_string(&aura_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: read {}: {e}", aura_path.display());
            return ExitCode::FAILURE;
        }
    };

    if scaffold::aura_toml_has_bundle(&aura_text, name) {
        eprintln!("error: aura.toml already lists bundle_id = \"{name}\"");
        return ExitCode::FAILURE;
    }

    let dest = PathBuf::from("plugins").join(name);
    if dest.exists() {
        eprintln!("error: {} already exists", dest.display());
        return ExitCode::FAILURE;
    }

    let spec = match make_spec(name, formats, kind) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    let crate_path = format!("plugins/{name}");
    let files = scaffold::plugin_crate_files(&spec);
    if let Err(e) = scaffold::write_files(&dest, &files) {
        eprintln!("error: scaffold failed: {e}");
        let _ = fs::remove_dir_all(&dest);
        return ExitCode::FAILURE;
    }

    let block = scaffold::plugin_table_block(
        &spec.display(),
        name,
        kind.category(),
        &crate_path,
    );
    let merged = scaffold::append_plugin_table(&aura_text, &block);
    if let Err(e) = fs::write(&aura_path, merged) {
        eprintln!("error: update {}: {e}", aura_path.display());
        let _ = fs::remove_dir_all(&dest);
        return ExitCode::FAILURE;
    }

    let mut all: Vec<&str> = vec!["clap"];
    all.extend(spec.formats.iter().map(String::as_str));
    let flags = all
        .iter()
        .map(|f| format!("--{f}"))
        .collect::<Vec<_>>()
        .join(" ");

    println!("added {}", dest.display());
    println!("  updated aura.toml  (+ [[plugin]] {name})");
    println!();
    println!("next:");
    println!("  cd plugins/{name}");
    println!("  cargo aura build {flags}");
    println!("  cargo aura install {flags} --release");
    ExitCode::SUCCESS
}

/// `cargo aura add-ui <name>` — scaffold a shared Slint UI crate under
/// `crates/<name>/` with a minimal theme + barrel. Intended for multi-plugin
/// workspaces that want a common design system (like `lx-ui-slint`).
fn cmd_add_ui(args: &[String]) -> ExitCode {
    if args.len() != 1 {
        eprintln!("usage: cargo aura add-ui <name>");
        return ExitCode::FAILURE;
    }
    let name = &args[0];

    if !scaffold::is_valid_crate_name(name) {
        eprintln!(
            "'{name}' is not a valid Cargo package name (use snake_case / kebab-case letters)"
        );
        return ExitCode::FAILURE;
    }

    let aura_path = PathBuf::from("aura.toml");
    if !aura_path.is_file() {
        eprintln!(
            "error: no aura.toml in current directory — run `cargo aura new` / `init` first, \
             then `add-ui` from that project root"
        );
        return ExitCode::FAILURE;
    }

    let dest = PathBuf::from("crates").join(name);
    if dest.exists() {
        eprintln!("error: {} already exists", dest.display());
        return ExitCode::FAILURE;
    }

    let aura_root = match aura_root() {
        Ok(r) => strip_verbatim_prefix(&r).to_string_lossy().replace('\\', "/"),
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    let files = scaffold::ui_crate_files(name, &aura_root);
    if let Err(e) = scaffold::write_files(&dest, &files) {
        eprintln!("error: scaffold failed: {e}");
        let _ = fs::remove_dir_all(&dest);
        return ExitCode::FAILURE;
    }

    println!("created {}", dest.display());
    println!("  Cargo.toml  build.rs  src/lib.rs  ui/{name}.slint  ui/{name}-theme.slint");
    println!();
    println!("next:");
    println!("  1. add \"crates/{name}\" to [workspace] members in ./Cargo.toml");
    println!("  2. import from plugins: import {{ ... }} from \"../../../crates/{name}/ui/{name}.slint\";");
    println!("  3. add your shared components to ui/{name}.slint");
    ExitCode::SUCCESS
}

/// `cargo aura gui` — launch the Slint project console (`aura-gui`).
fn cmd_gui() -> ExitCode {
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
        .arg("aura-gui")
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"));
    match cmd.status() {
        Ok(s) if s.success() => ExitCode::SUCCESS,
        Ok(s) => ExitCode::from(u8::try_from(s.code().unwrap_or(1)).unwrap_or(1)),
        Err(e) => {
            eprintln!("failed to run cargo: {e}");
            ExitCode::FAILURE
        }
    }
}

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
    let parsed = match parse_build_args(args) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!(
                "usage: cargo aura build [--clap|--vst3|--lv2] [--release] [-plug <crate> [<crate>...]] [cargo options]"
            );
            return ExitCode::FAILURE;
        }
    };

    if parsed.formats.is_empty() {
        eprintln!("note: no --clap/--vst3/--lv2; building default features");
    }

    if parsed.plugins.is_empty() {
        build_one(None, &parsed.formats, parsed.release, &parsed.rest)
    } else {
        for plugin in &parsed.plugins {
            if build_one(Some(plugin), &parsed.formats, parsed.release, &parsed.rest)
                != ExitCode::SUCCESS
            {
                return ExitCode::FAILURE;
            }
        }
        ExitCode::SUCCESS
    }
}

fn build_one(
    plugin: Option<&str>,
    features: &[String],
    release: bool,
    rest: &[String],
) -> ExitCode {
    let mut cmd = Command::new("cargo");
    cmd.arg("build");
    if release {
        cmd.arg("--release");
    }
    if let Some(p) = plugin {
        cmd.arg("-p").arg(p);
    }
    if !features.is_empty() {
        cmd.arg("--features");
        cmd.arg(features.join(","));
    }
    cmd.args(rest);

    eprintln!(
        "--- cargo build {}---",
        plugin.map_or(String::new(), |p| format!("-p {p} "))
    );
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
    let parsed = match parse_build_args(args) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!(
                "usage: cargo aura install --clap|--vst3|--lv2 [--release] [-plug <crate> [<crate>...]]"
            );
            return ExitCode::FAILURE;
        }
    };

    if parsed.formats.is_empty() {
        eprintln!(
            "usage: cargo aura install --clap|--vst3|--lv2 [--release] [-plug <crate> [<crate>...]]"
        );
        return ExitCode::FAILURE;
    }

    let profile = if parsed.release { "release" } else { "debug" };
    let target_dir = project_target_dir();

    if parsed.plugins.is_empty() {
        // Legacy single-crate behaviour: build the current crate.
        let mut build_args: Vec<String> = parsed.formats.iter().map(|f| format!("--{f}")).collect();
        if parsed.release {
            build_args.push("--release".into());
        }
        build_args.extend(parsed.rest.clone());
        if cmd_build(&build_args) != ExitCode::SUCCESS {
            return ExitCode::FAILURE;
        }

        let Some(pkg) = package_name() else {
            eprintln!("error: could not read [package] name from ./Cargo.toml");
            return ExitCode::FAILURE;
        };
        install_formats(&pkg, &target_dir, profile, &parsed.formats)
    } else {
        for plugin in &parsed.plugins {
            let mut build_args = vec!["-plug".to_string(), plugin.clone()];
            for f in &parsed.formats {
                build_args.push(format!("--{f}"));
            }
            if parsed.release {
                build_args.push("--release".into());
            }
            build_args.extend(parsed.rest.clone());
            if cmd_build(&build_args) != ExitCode::SUCCESS {
                return ExitCode::FAILURE;
            }
            if install_formats(plugin, &target_dir, profile, &parsed.formats)
                != ExitCode::SUCCESS
            {
                return ExitCode::FAILURE;
            }
        }
        ExitCode::SUCCESS
    }
}

fn install_formats(pkg: &str, target_dir: &Path, profile: &str, features: &[String]) -> ExitCode {
    for feat in features {
        let res = match feat.as_str() {
            "clap" => install_clap(pkg, target_dir, profile),
            "vst3" => install_vst3(pkg, target_dir, profile),
            "lv2" => install_lv2(pkg, target_dir, profile),
            _ => Ok(()),
        };
        if let Err(e) = res {
            eprintln!("install --{feat} for {pkg}: {e}");
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

fn install_clap(pkg: &str, target_dir: &Path, profile: &str) -> Result<(), String> {
    let dir = target_dir.join(profile);
    if !dir.is_dir() {
        return Err(format!("no build dir {}", dir.display()));
    }

    // Ship exactly this package's cdylib as `<package>.clap` — never a
    // random dependency artifact that happens to sit in the target dir.
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

    let dest_root = resolve_install_dir(InstallFormat::Clap)?;
    fs::create_dir_all(&dest_root).map_err(|e| {
        format!(
            "create_dir_all {}: {e} (resolved install dir — check aura.toml [install])",
            dest_root.display()
        )
    })?;

    let display = plugin_display_name(pkg);
    let dest = dest_root.join(format!("{display}.clap"));
    fs::copy(src, &dest).map_err(|e| format!("copy {} → {}: {e}", src.display(), dest.display()))?;
    println!("installed {}", dest.display());
    Ok(())
}

/// Install as a VST3 module bundle:
/// ```text
/// <name>.vst3/Contents/<arch>/<name>.vst3   # renamed cdylib
/// ```
/// Arch folder follows Steinberg: `x86_64-win`, `x86_64-linux`, `MacOS`, …
fn install_vst3(pkg: &str, target_dir: &Path, profile: &str) -> Result<(), String> {
    let dir = target_dir.join(profile);
    if !dir.is_dir() {
        return Err(format!("no build dir {}", dir.display()));
    }

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

    let display = plugin_display_name(pkg);
    let dest_root = resolve_install_dir(InstallFormat::Vst3)?;
    let bundle = dest_root.join(format!("{display}.vst3"));
    let arch_dir = bundle.join("Contents").join(vst3_arch_folder());
    fs::create_dir_all(&arch_dir).map_err(|e| {
        format!(
            "create_dir_all {}: {e} (resolved install dir — check aura.toml [install])",
            arch_dir.display()
        )
    })?;

    // Binary inside the bundle uses the `.vst3` extension (still a PE/ELF/Mach-O).
    let dest = arch_dir.join(format!("{display}.vst3"));
    fs::copy(src, &dest).map_err(|e| format!("copy {} → {}: {e}", src.display(), dest.display()))?;
    println!("installed {}", bundle.display());
    Ok(())
}

/// Install as an LV2 bundle:
/// ```text
/// <name>.lv2/
///   manifest.ttl
///   plugin.ttl
///   <binary>          # package stem, platform library name
/// ```
fn install_lv2(pkg: &str, target_dir: &Path, profile: &str) -> Result<(), String> {
    let dir = target_dir.join(profile);
    if !dir.is_dir() {
        return Err(format!("no build dir {}", dir.display()));
    }

    let crate_name = pkg.replace('-', "_");
    let candidates = find_plugin_artifacts(&dir)?;
    let src = candidates
        .iter()
        .find(|p| artifact_stem(p) == Some(crate_name.as_str()))
        .ok_or_else(|| {
            format!(
                "no built artifact for `{pkg}` in {} — crate-type cdylib + `cargo aura build --lv2` first",
                dir.display()
            )
        })?;

    // TTL: prefer calling into aura-lv2 helpers via a tiny generated sidecar
    // written by the plugin feature would be ideal; for install we regenerate
    // from aura.toml plugin name + a minimal default if smoke-style.
    // Real TTL comes from `aura_lv2::bundle_ttl` when we can link it — cargo-aura
    // is a tool without plugin monomorphization. Generate a workable stereo
    // gain-style TTL from package metadata; plugins with more params should
    // ship prebuilt TTL later or we grow a build-script emit.
    //
    // For now: write TTL using package name + generic stereo+1 control "gain"
    // only when aura.toml lacks ports — actually better: shell out is wrong.
    // Install copies binary + writes TTL from a JSON sidecar if present, else
    // invokes the well-known layout for smoke-gain-class plugins.
    //
    // Practical v1: regenerate via `aura_lv2` types by embedding a small
    // template. smoke-gain has gain id=1 -24..24. Multi-param plugins get
    // ports listed in aura.toml later.
    let display = plugin_display_name(pkg);
    let dest_root = resolve_install_dir(InstallFormat::Lv2)?;
    let bundle = dest_root.join(format!("{display}.lv2"));
    fs::create_dir_all(&bundle).map_err(|e| e.to_string())?;

    let binary_name = src
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("artifact has no file name")?
        .to_string();

    // Prefer TTL produced at build time if the plugin left one next to the artifact.
    let ttl_dir = dir.join(format!("{pkg}-lv2-ttl"));
    let (manifest, plugin_ttl) = if ttl_dir.join("manifest.ttl").is_file() {
        (
            fs::read_to_string(ttl_dir.join("manifest.ttl")).map_err(|e| e.to_string())?,
            fs::read_to_string(ttl_dir.join("plugin.ttl")).map_err(|e| e.to_string())?,
        )
    } else {
        // Fallback template (stereo FX + optional gain control) — good enough
        // for smoke-gain; authors can place `{pkg}-lv2-ttl/` after a future
        // `cargo aura lv2-ttl` command.
        lv2_fallback_ttl(pkg, &binary_name)
    };

    // Rewrite binary name in manifest if the fallback used a placeholder.
    let manifest = manifest.replace("BINARY_PLACEHOLDER", &binary_name);

    fs::write(bundle.join("manifest.ttl"), manifest).map_err(|e| e.to_string())?;
    fs::write(bundle.join("plugin.ttl"), plugin_ttl).map_err(|e| e.to_string())?;

    let dest_bin = bundle.join(&binary_name);
    fs::copy(src, &dest_bin)
        .map_err(|e| format!("copy {} → {}: {e}", src.display(), dest_bin.display()))?;
    println!("installed {}", bundle.display());
    Ok(())
}

/// Minimal stereo-FX TTL when no build-time sidecar exists.
#[allow(clippy::needless_raw_string_hashes, clippy::uninlined_format_args)]
fn lv2_fallback_ttl(pkg: &str, binary_name: &str) -> (String, String) {
    let uri = format!("https://lx-audiolabs.com/lv2/{pkg}");
    let manifest = format!(
        "\
@prefix lv2:  <http://lv2plug.in/ns/lv2core#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .

<{uri}>
    a lv2:Plugin ;
    lv2:binary <{binary_name}> ;
    rdfs:seeAlso <plugin.ttl> .
"
    );
    let plugin = format!(
        "\
@prefix doap:  <http://usefulinc.com/ns/doap#> .
@prefix lv2:   <http://lv2plug.in/ns/lv2core#> .
@prefix rdfs:  <http://www.w3.org/2000/01/rdf-schema#> .
@prefix state: <http://lv2plug.in/ns/ext/state#> .

<{uri}>
    a lv2:Plugin, lv2:AmplifierPlugin ;
    doap:name \"{pkg}\" ;
    doap:license <https://spdx.org/licenses/GPL-3.0-or-later> ;
    lv2:optionalFeature lv2:hardRTCapable ;
    lv2:extensionData state:interface ;
    lv2:port [
        a lv2:InputPort, lv2:AudioPort ;
        lv2:index 0 ;
        lv2:symbol \"in_l\" ;
        lv2:name \"Input L\" ;
    ] , [
        a lv2:InputPort, lv2:AudioPort ;
        lv2:index 1 ;
        lv2:symbol \"in_r\" ;
        lv2:name \"Input R\" ;
    ] , [
        a lv2:OutputPort, lv2:AudioPort ;
        lv2:index 2 ;
        lv2:symbol \"out_l\" ;
        lv2:name \"Output L\" ;
    ] , [
        a lv2:OutputPort, lv2:AudioPort ;
        lv2:index 3 ;
        lv2:symbol \"out_r\" ;
        lv2:name \"Output R\" ;
    ] , [
        a lv2:InputPort, lv2:ControlPort ;
        lv2:index 4 ;
        lv2:symbol \"gain\" ;
        lv2:name \"Gain\" ;
        lv2:default 0.0 ;
        lv2:minimum -24.0 ;
        lv2:maximum 24.0 ;
    ] .
"
    );
    (manifest, plugin)
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

/// Format-specific install destination.
#[derive(Clone, Copy)]
enum InstallFormat {
    Clap,
    Vst3,
    #[allow(dead_code)] // used when LV2 install lands
    Lv2,
}

impl InstallFormat {
    fn env_keys(self) -> &'static [&'static str] {
        match self {
            Self::Clap => &["CLAPINS", "CLAP_PATH"],
            Self::Vst3 => &["VST3INS", "VST3_PATH"],
            Self::Lv2 => &["LV2INS", "LV2_PATH"],
        }
    }

    fn toml_key(self) -> &'static str {
        match self {
            Self::Clap => "clap",
            Self::Vst3 => "vst3",
            Self::Lv2 => "lv2",
        }
    }

    fn subdir(self) -> &'static str {
        match self {
            Self::Clap => "CLAP",
            Self::Vst3 => "VST3",
            Self::Lv2 => "LV2",
        }
    }
}

/// Resolve install root for a format.
///
/// Order: env override → `aura.toml` per-format path → `aura.toml` `[install].dir`
/// + format subdir → OS host default.
fn resolve_install_dir(fmt: InstallFormat) -> Result<PathBuf, String> {
    for key in fmt.env_keys() {
        if let Ok(p) = env::var(key)
            && !p.trim().is_empty()
        {
            return expand_path(p.trim());
        }
    }

    let cfg = read_aura_install_config();
    if let Some(p) = cfg.get(fmt.toml_key()) {
        return expand_path(p);
    }
    if let Some(base) = cfg.get("dir").or_else(|| cfg.get("installdir")) {
        return Ok(expand_path(base)?.join(fmt.subdir()));
    }

    default_install_dir(fmt)
}

/// Line-scan `./aura.toml` `[install]` table — no toml crate needed.
/// Keys: `dir`, `installdir`, `clap`, `vst3`, `lv2`.
fn read_aura_install_config() -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    let Ok(text) = fs::read_to_string("aura.toml") else {
        return out;
    };
    let mut in_install = false;
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            in_install = line == "[install]";
            continue;
        }
        if !in_install {
            continue;
        }
        let Some((key, val)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let val = val.trim().trim_matches('"').trim_matches('\'').to_string();
        if matches!(
            key.as_str(),
            "dir" | "installdir" | "clap" | "vst3" | "lv2"
        ) && !val.is_empty()
        {
            out.insert(key, val);
        }
    }
    out
}

/// Look up an environment variable; on Windows, match case-insensitively.
fn env_lookup(name: &str) -> Option<String> {
    if let Ok(v) = env::var(name) {
        return Some(v);
    }
    // Windows env block is case-insensitive; some hosts only expose mixed case.
    let want = name.to_ascii_uppercase();
    env::vars()
        .find(|(k, _)| k.to_ascii_uppercase() == want)
        .map(|(_, v)| v)
}

/// Expand `%VAR%`, `$VAR` / `${VAR}`, and leading `~` in install paths.
///
/// Unresolved `%VAR%` segments are **not** left in place (that created
/// relative folders literally named `%LOCALAPPDATA%` under the project).
fn expand_path(raw: &str) -> Result<PathBuf, String> {
    let mut s = raw.to_string();

    // Normalize TOML-style doubled backslashes from line-scan reads.
    s = s.replace("\\\\", "\\");

    // Windows-style %VAR% — replace until none left.
    while let Some(start) = s.find('%') {
        let rest = &s[start + 1..];
        let Some(end_rel) = rest.find('%') else {
            return Err(format!(
                "install path has unclosed %…%: {raw:?} (after partial expand: {s:?})"
            ));
        };
        if end_rel == 0 {
            return Err(format!("install path has empty %…%: {raw:?}"));
        }
        let name = &rest[..end_rel];
        if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(format!(
                "install path has invalid env name %{name}% in {raw:?}"
            ));
        }
        let Some(repl) = env_lookup(name) else {
            return Err(format!(
                "install path env %{name}% is not set (from {raw:?}). \
                 Set the variable or use a concrete path in aura.toml [install]"
            ));
        };
        let end = start + 1 + end_rel + 1; // inclusive of closing %
        s.replace_range(start..end, &repl);
    }

    // $VAR or ${VAR}
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '$' && i + 1 < chars.len() {
            if chars[i + 1] == '{' {
                if let Some(close) = chars[i + 2..].iter().position(|&c| c == '}') {
                    let name: String = chars[i + 2..i + 2 + close].iter().collect();
                    let repl = env_lookup(&name).ok_or_else(|| {
                        format!("install path env ${{{name}}} is not set (from {raw:?})")
                    })?;
                    out.push_str(&repl);
                    i += 3 + close;
                    continue;
                }
            } else if chars[i + 1].is_ascii_alphabetic() || chars[i + 1] == '_' {
                let mut j = i + 1;
                while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_') {
                    j += 1;
                }
                let name: String = chars[i + 1..j].iter().collect();
                let repl = env_lookup(&name).ok_or_else(|| {
                    format!("install path env ${name} is not set (from {raw:?})")
                })?;
                out.push_str(&repl);
                i = j;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    s = out;

    if s.starts_with("~/") || s.starts_with("~\\") {
        let home = env_lookup("HOME")
            .or_else(|| env_lookup("USERPROFILE"))
            .ok_or_else(|| "cannot expand ~: HOME/USERPROFILE not set".to_string())?;
        s = format!("{home}{}", &s[1..]);
    } else if s == "~" {
        s = env_lookup("HOME")
            .or_else(|| env_lookup("USERPROFILE"))
            .ok_or_else(|| "cannot expand ~: HOME/USERPROFILE not set".to_string())?;
    }

    if s.contains('%') {
        return Err(format!(
            "install path still contains % after expand: {s:?} (from {raw:?})"
        ));
    }

    Ok(PathBuf::from(s))
}

fn default_install_dir(fmt: InstallFormat) -> Result<PathBuf, String> {
    match fmt {
        InstallFormat::Clap => default_clap_install_dir(),
        InstallFormat::Vst3 => default_vst3_install_dir(),
        InstallFormat::Lv2 => default_lv2_install_dir(),
    }
}

#[allow(clippy::unnecessary_wraps)]
fn default_vst3_install_dir() -> Result<PathBuf, String> {
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
        Err("unsupported OS for default VST3 path; set VST3INS or [install] in aura.toml".into())
    }
}

#[allow(clippy::unnecessary_wraps)]
fn default_lv2_install_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(local) = env::var("LOCALAPPDATA") {
            return Ok(PathBuf::from(local).join("LV2"));
        }
        Ok(PathBuf::from(r"C:\Program Files\Common Files\LV2"))
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = env::var("HOME") {
            return Ok(PathBuf::from(home).join("Library/Audio/Plug-Ins/LV2"));
        }
        Err("HOME not set".into())
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(home) = env::var("HOME") {
            return Ok(PathBuf::from(home).join(".lv2"));
        }
        Err("HOME not set".into())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err("unsupported OS for default LV2 path; set LV2INS or [install] in aura.toml".into())
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

/// Display `name` for the `[[plugin]]` entry whose `crate` matches `pkg`.
/// Falls back to `pkg` if no aura.toml or no matching entry is found.
fn plugin_display_name(pkg: &str) -> String {
    match fs::read_to_string("aura.toml") {
        Ok(text) => plugin_display_name_from_text(&text, pkg),
        Err(_) => pkg.to_string(),
    }
}

fn plugin_display_name_from_text(text: &str, pkg: &str) -> String {
    let mut in_plugin = false;
    let mut current_crate: Option<String> = None;
    let mut current_name: Option<String> = None;
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.starts_with("[[plugin]]") {
            if current_crate.as_deref() == Some(pkg)
                && let Some(name) = current_name
            {
                return name;
            }
            in_plugin = true;
            current_crate = None;
            current_name = None;
            continue;
        }
        if line.starts_with('[') && !line.starts_with("[[") {
            in_plugin = false;
            continue;
        }
        if !in_plugin {
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            let key = key.trim();
            let val = val.trim().trim_matches('"').trim_matches('\'').to_string();
            match key {
                "crate" => current_crate = Some(val),
                "name" => current_name = Some(val),
                _ => {}
            }
        }
    }
    if current_crate.as_deref() == Some(pkg)
        && let Some(name) = current_name
    {
        return name;
    }
    pkg.to_string()
}

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
fn default_clap_install_dir() -> Result<PathBuf, String> {
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
        Err("unsupported OS for default CLAP path; set CLAPINS or [install] in aura.toml".into())
    }
}

fn project_target_dir() -> PathBuf {
    // Respect CARGO_TARGET_DIR; else ask cargo — a plugin that is a workspace
    // member builds into the *workspace root's* target/, not ./target.
    if let Some(dir) = env::var_os("CARGO_TARGET_DIR") {
        return PathBuf::from(dir);
    }
    metadata_target_dir().unwrap_or_else(|| PathBuf::from("target"))
}

/// `target_directory` from `cargo metadata` (string scan — no json dep needed).
fn metadata_target_dir() -> Option<PathBuf> {
    let out = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?;
    let key = "\"target_directory\":\"";
    let start = text.find(key)? + key.len();
    let rest = &text[start..];
    let end = rest.find('"')?;
    // JSON escapes backslashes on Windows ("C:\\foo") — collapse \\\\ pairs.
    Some(PathBuf::from(rest[..end].replace("\\\\", "\\")))
}

/// Parsed `build` / `install` args.
#[derive(Debug, Clone, PartialEq, Eq)]
struct BuildArgs {
    formats: Vec<String>,
    release: bool,
    /// Plugins selected via `-plug <name> [<name> ...]`. Empty means
    /// "current crate" (legacy single-crate behaviour).
    plugins: Vec<String>,
    /// Anything else left over.
    rest: Vec<String>,
}

/// Parse `build` / `install` flags plus the `-plug <crate> [<crate>...]`
/// multi-plugin selector. `-plug` consumes every following positional arg
/// until the next flag (`-` prefix). Use `--` to stop option parsing if a
/// crate name starts with `-`.
fn parse_build_args(args: &[String]) -> Result<BuildArgs, String> {
    let mut formats = Vec::new();
    let mut release = false;
    let mut plugins = Vec::new();
    let mut rest = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "--clap" => formats.push("clap".into()),
            "--vst3" => formats.push("vst3".into()),
            "--lv2" => formats.push("lv2".into()),
            "--release" => release = true,
            "-plug" => {
                i += 1;
                let start = i;
                while i < args.len() && !args[i].starts_with('-') {
                    plugins.push(args[i].clone());
                    i += 1;
                }
                if plugins.is_empty() || i == start {
                    return Err("-plug needs at least one crate name".into());
                }
                continue;
            }
            other => rest.push(other.to_string()),
        }
        i += 1;
    }

    Ok(BuildArgs {
        formats,
        release,
        plugins,
        rest,
    })
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

/// Drop Windows extended-length prefix (`\\?\`) so path deps work in Cargo.toml.
fn strip_verbatim_prefix(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_path_percent_vars() {
        // SAFETY: test-only env; single-threaded unit test.
        unsafe { env::set_var("AURA_TEST_INSTALL", r"C:\Users\test\AppData\Local") };
        let p = expand_path(r"%AURA_TEST_INSTALL%\Programs\Common").unwrap();
        assert_eq!(
            p,
            PathBuf::from(r"C:\Users\test\AppData\Local\Programs\Common")
        );
        // Double backslashes as written in aura.toml line-scan values.
        let p2 = expand_path(r"%AURA_TEST_INSTALL%\\Programs\\Common").unwrap();
        assert_eq!(
            p2,
            PathBuf::from(r"C:\Users\test\AppData\Local\Programs\Common")
        );
        unsafe { env::remove_var("AURA_TEST_INSTALL") };
    }

    #[test]
    fn expand_path_dollar_and_tilde() {
        unsafe { env::set_var("AURA_TEST_HOME", "/home/dev") };
        let p = expand_path("$AURA_TEST_HOME/.clap").unwrap();
        assert_eq!(p, PathBuf::from("/home/dev/.clap"));
        let p2 = expand_path("${AURA_TEST_HOME}/.vst3").unwrap();
        assert_eq!(p2, PathBuf::from("/home/dev/.vst3"));
        unsafe { env::remove_var("AURA_TEST_HOME") };
    }

    #[test]
    fn expand_path_unknown_var_errors() {
        let err = expand_path(r"%AURA_SURELY_UNSET_XYZ%\foo").unwrap_err();
        assert!(err.contains("AURA_SURELY_UNSET_XYZ"), "{err}");
    }

    #[test]
    fn expand_localappdata_if_present() {
        if env::var("LOCALAPPDATA").is_err() {
            return;
        }
        // Exact scaffold default shape.
        let p = expand_path(r"%LOCALAPPDATA%\\Programs\\Common").unwrap();
        let s = p.to_string_lossy();
        assert!(!s.contains('%'), "{s}");
        assert!(s.ends_with(r"Programs\Common") || s.ends_with("Programs/Common"), "{s}");
    }

    #[test]
    fn parse_build_args_formats_and_release() {
        let args = vec!["--clap".into(), "--vst3".into(), "--release".into()];
        let p = parse_build_args(&args).unwrap();
        assert_eq!(p.formats, vec!["clap", "vst3"]);
        assert!(p.release);
        assert!(p.plugins.is_empty());
        assert!(p.rest.is_empty());
    }

    #[test]
    fn parse_build_args_single_plug() {
        let args = vec!["--clap".into(), "-plug".into(), "aether".into()];
        let p = parse_build_args(&args).unwrap();
        assert_eq!(p.formats, vec!["clap"]);
        assert_eq!(p.plugins, vec!["aether"]);
        assert!(!p.release);
    }

    #[test]
    fn parse_build_args_multi_plug() {
        let args = vec![
            "--clap".into(),
            "--vst3".into(),
            "-plug".into(),
            "aether".into(),
            "meridian".into(),
            "equilibrium".into(),
            "--release".into(),
        ];
        let p = parse_build_args(&args).unwrap();
        assert_eq!(p.formats, vec!["clap", "vst3"]);
        assert!(p.release);
        assert_eq!(p.plugins, vec!["aether", "meridian", "equilibrium"]);
        assert!(p.rest.is_empty());
    }

    #[test]
    fn parse_build_args_plug_stops_at_next_flag() {
        let args = vec![
            "-plug".into(),
            "aether".into(),
            "meridian".into(),
            "--clap".into(),
        ];
        let p = parse_build_args(&args).unwrap();
        assert_eq!(p.plugins, vec!["aether", "meridian"]);
        assert_eq!(p.formats, vec!["clap"]);
    }

    #[test]
    fn parse_build_args_plug_without_name_errors() {
        let args = vec!["--clap".into(), "-plug".into()];
        assert!(parse_build_args(&args).is_err());
    }

    #[test]
    fn parse_build_args_passes_through_cargo_options() {
        let args = vec![
            "--clap".into(),
            "-plug".into(),
            "aether".into(),
            "--target".into(),
            "x86_64-unknown-linux-gnu".into(),
        ];
        let p = parse_build_args(&args).unwrap();
        assert_eq!(p.formats, vec!["clap"]);
        assert_eq!(p.plugins, vec!["aether"]);
        assert_eq!(p.rest, vec!["--target", "x86_64-unknown-linux-gnu"]);
    }

    #[test]
    fn plugin_display_name_from_aura_toml() {
        let text = r#"
[[plugin]]
name = "Aether"
bundle_id = "aether"
crate = "aether"
category = "effect"

[[plugin]]
name = "Lucent Relay"
bundle_id = "lucentrelay"
crate = "lucent-relay"
category = "analyzer"
"#;
        assert_eq!(plugin_display_name_from_text(text, "aether"), "Aether");
        assert_eq!(
            plugin_display_name_from_text(text, "lucent-relay"),
            "Lucent Relay"
        );
        assert_eq!(plugin_display_name_from_text(text, "unknown"), "unknown");
    }
}
