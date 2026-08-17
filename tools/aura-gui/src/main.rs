//! Visual AURA project console.
//!
//! Thin Slint shell over the same operations as `cargo aura …`.
//! Long work runs on a worker thread; the UI only enqueues commands.
//!
//! ```bash
//! cargo run -p aura-gui
//! # or: cargo aura gui
//! ```

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

use slint::{ComponentHandle, SharedString, Weak};

slint::include_modules!();

type JobBuilder =
    Arc<dyn Fn(&UiState) -> Result<(String, Vec<String>), String> + Send + Sync + 'static>;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = AppWindow::new()?;
    let ui_weak = ui.as_weak();
    let busy = Arc::new(Mutex::new(false));

    if let Ok(cwd) = std::env::current_dir() {
        ui.set_project_dir(path_to_shared(&cwd));
    }

    {
        let ui_w = ui_weak.clone();
        ui.on_clear_log(move || {
            if let Some(ui) = ui_w.upgrade() {
                ui.set_log_text(SharedString::from(""));
            }
        });
    }

    {
        let b = job_builder(|s| {
            require_name(s)?;
            let mut args = vec!["new".into(), s.plugin_name.clone()];
            push_scaffold_flags(&mut args, s);
            Ok((s.project_dir.clone(), args))
        });
        let ui_w = ui_weak.clone();
        let busy = Arc::clone(&busy);
        ui.on_run_new(move || spawn_job(ui_w.clone(), Arc::clone(&busy), Arc::clone(&b)));
    }
    {
        let b = job_builder(|s| {
            let mut args = vec!["init".into(), s.project_dir.clone()];
            push_scaffold_flags(&mut args, s);
            Ok((".".into(), args))
        });
        let ui_w = ui_weak.clone();
        let busy = Arc::clone(&busy);
        ui.on_run_init(move || spawn_job(ui_w.clone(), Arc::clone(&busy), Arc::clone(&b)));
    }
    {
        let b = job_builder(|s| {
            require_name(s)?;
            let mut args = vec!["add".into(), s.plugin_name.clone()];
            push_scaffold_flags(&mut args, s);
            Ok((s.project_dir.clone(), args))
        });
        let ui_w = ui_weak.clone();
        let busy = Arc::clone(&busy);
        ui.on_run_add(move || spawn_job(ui_w.clone(), Arc::clone(&busy), Arc::clone(&b)));
    }
    {
        let b = job_builder(|s| {
            let mut args = vec!["build".into()];
            push_build_flags(&mut args, s);
            Ok((s.project_dir.clone(), args))
        });
        let ui_w = ui_weak.clone();
        let busy = Arc::clone(&busy);
        ui.on_run_build(move || spawn_job(ui_w.clone(), Arc::clone(&busy), Arc::clone(&b)));
    }
    {
        let b = job_builder(|s| {
            if !s.format_clap && !s.format_vst3 && !s.format_lv2 {
                return Err("install needs at least one format (CLAP / VST3 / LV2)".into());
            }
            let mut args = vec!["install".into()];
            push_build_flags(&mut args, s);
            Ok((s.project_dir.clone(), args))
        });
        let ui_w = ui_weak.clone();
        let busy = Arc::clone(&busy);
        ui.on_run_install(move || spawn_job(ui_w.clone(), Arc::clone(&busy), Arc::clone(&b)));
    }
    {
        let b = job_builder(|s| Ok((s.project_dir.clone(), vec!["doctor".into()])));
        let ui_w = ui_weak.clone();
        let busy = Arc::clone(&busy);
        ui.on_run_doctor(move || spawn_job(ui_w.clone(), Arc::clone(&busy), Arc::clone(&b)));
    }
    {
        let b = job_builder(|s| Ok((s.project_dir.clone(), vec!["mesh".into()])));
        let ui_w = ui_weak.clone();
        let busy = Arc::clone(&busy);
        ui.on_run_mesh(move || spawn_job(ui_w.clone(), Arc::clone(&busy), Arc::clone(&b)));
    }

    invoke_log(&ui_weak, &format!("AURA root hint: {}\n", aura_root_hint()));
    ui.run()?;
    Ok(())
}

fn job_builder<F>(f: F) -> JobBuilder
where
    F: Fn(&UiState) -> Result<(String, Vec<String>), String> + Send + Sync + 'static,
{
    Arc::new(f)
}

fn spawn_job(ui_w: Weak<AppWindow>, busy: Arc<Mutex<bool>>, builder: JobBuilder) {
    let Some(ui) = ui_w.upgrade() else {
        return;
    };
    {
        let mut b = busy.lock().unwrap_or_else(|e| e.into_inner());
        if *b {
            append_log_ui(&ui, "already busy — wait for the current command\n");
            return;
        }
        *b = true;
    }
    ui.set_busy(true);
    let state = snapshot(&ui);

    thread::spawn(move || {
        match builder(&state) {
            Ok((cwd, args)) => {
                invoke_log(&ui_w, &format!("$ cargo aura {}\n", args.join(" ")));
                match run_cargo_aura(&cwd, &args, |chunk| invoke_log(&ui_w, chunk)) {
                    Ok(code) => invoke_log(&ui_w, &format!("\n— exit {code} —\n")),
                    Err(e) => invoke_log(&ui_w, &format!("error: {e}\n")),
                }
            }
            Err(e) => invoke_log(&ui_w, &format!("error: {e}\n")),
        }

        let ui_done = ui_w.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_done.upgrade() {
                ui.set_busy(false);
            }
        });
        if let Ok(mut b) = busy.lock() {
            *b = false;
        }
    });
}

// ---------------------------------------------------------------------------
// UI state
// ---------------------------------------------------------------------------

struct UiState {
    project_dir: String,
    plugin_name: String,
    kind_index: i32,
    format_clap: bool,
    format_vst3: bool,
    format_lv2: bool,
    format_hot: bool,
    release_build: bool,
}

fn snapshot(ui: &AppWindow) -> UiState {
    UiState {
        project_dir: ui.get_project_dir().to_string(),
        plugin_name: ui.get_plugin_name().to_string(),
        kind_index: ui.get_kind_index(),
        format_clap: ui.get_format_clap(),
        format_vst3: ui.get_format_vst3(),
        format_lv2: ui.get_format_lv2(),
        format_hot: ui.get_format_hot(),
        release_build: ui.get_release_build(),
    }
}

fn require_name(s: &UiState) -> Result<(), String> {
    if s.plugin_name.trim().is_empty() {
        Err("plugin name is empty".into())
    } else {
        Ok(())
    }
}

fn kind_flag(index: i32) -> &'static str {
    match index {
        1 => "effect-mono",
        2 => "analyzer",
        _ => "effect",
    }
}

fn push_scaffold_flags(args: &mut Vec<String>, s: &UiState) {
    if s.format_vst3 {
        args.push("--vst3".into());
    }
    if s.format_lv2 {
        args.push("--lv2".into());
    }
    args.push("--kind".into());
    args.push(kind_flag(s.kind_index).into());
}

fn push_build_flags(args: &mut Vec<String>, s: &UiState) {
    if s.format_clap {
        args.push("--clap".into());
    }
    if s.format_vst3 {
        args.push("--vst3".into());
    }
    if s.format_lv2 {
        args.push("--lv2".into());
    }
    if !s.format_clap && !s.format_vst3 && !s.format_lv2 {
        args.push("--clap".into());
    }
    if s.release_build {
        args.push("--release".into());
    }
    if s.format_hot {
        args.push("--hot".into());
    }
}

// ---------------------------------------------------------------------------
// cargo aura
// ---------------------------------------------------------------------------

fn run_cargo_aura(
    project_dir: &str,
    args: &[String],
    mut on_line: impl FnMut(&str),
) -> Result<i32, String> {
    let cwd = PathBuf::from(project_dir);

    let mut cmd = resolve_cargo_aura()?;
    cmd.args(args);
    if cwd.is_dir() {
        cmd.current_dir(&cwd);
    } else if let Some(parent) = cwd.parent().filter(|p| p.is_dir()) {
        cmd.current_dir(parent);
    }

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("spawn cargo aura failed: {e}\n{}", install_hint()))?;

    let (tx, rx) = std::sync::mpsc::channel::<String>();
    if let Some(out) = child.stdout.take() {
        let tx = tx.clone();
        thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                let _ = tx.send(format!("{line}\n"));
            }
        });
    }
    if let Some(err) = child.stderr.take() {
        let tx = tx.clone();
        thread::spawn(move || {
            for line in BufReader::new(err).lines().map_while(Result::ok) {
                let _ = tx.send(format!("{line}\n"));
            }
        });
    }
    drop(tx);

    while let Ok(chunk) = rx.recv() {
        on_line(&chunk);
    }

    let status = child.wait().map_err(|e| format!("wait failed: {e}"))?;
    Ok(status.code().unwrap_or(1))
}

fn resolve_cargo_aura() -> Result<Command, String> {
    if which("cargo-aura").is_some() {
        let mut c = Command::new("cargo");
        c.arg("aura");
        return Ok(c);
    }
    if let Some(root) = find_aura_root() {
        let mut c = Command::new("cargo");
        c.arg("run")
            .arg("-q")
            .arg("-p")
            .arg("cargo-aura")
            .arg("--manifest-path")
            .arg(root.join("Cargo.toml"))
            .arg("--");
        return Ok(c);
    }
    Err(format!("cargo-aura not found\n{}", install_hint()))
}

fn install_hint() -> String {
    "install: cargo install --path tools/cargo-aura --force\n\
     or set AURA_PATH / run from the AURA workspace"
        .into()
}

fn find_aura_root() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("AURA_PATH") {
        let p = PathBuf::from(p);
        if p.join("crates/aura").exists() {
            return Some(p);
        }
    }
    let mut dir = std::env::current_dir().ok()?;
    for _ in 0..10 {
        if dir.join("crates/aura").is_dir() && dir.join("tools/cargo-aura").is_dir() {
            return Some(dir);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

fn aura_root_hint() -> String {
    find_aura_root()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(set AURA_PATH or install cargo-aura)".into())
}

fn which(bin: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
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

// ---------------------------------------------------------------------------
// log
// ---------------------------------------------------------------------------

fn path_to_shared(p: &Path) -> SharedString {
    SharedString::from(p.to_string_lossy().as_ref())
}

fn append_log_ui(ui: &AppWindow, chunk: &str) {
    let mut t = ui.get_log_text().to_string();
    t.push_str(chunk);
    const MAX: usize = 80_000;
    if t.len() > MAX {
        t = t[t.len() - MAX..].to_string();
    }
    ui.set_log_text(SharedString::from(t));
}

fn invoke_log(ui: &Weak<AppWindow>, chunk: &str) {
    let ui = ui.clone();
    let chunk = chunk.to_string();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui.upgrade() {
            append_log_ui(&ui, &chunk);
        }
    });
}
