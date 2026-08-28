//! aura-preview — hot-reload preview for AURA plugin `.slint` UIs.
//!
//! Renders a plugin's `ui/main.slint` with the same `@aura` widget library,
//! bundled fonts and fluent style as the real build — without compiling the
//! plugin. Reload via the button or automatically on save.
//!
//! Usage:
//!   aura-preview [path/to/main.slint] [--component <Name>] [--no-watch]

use std::cell::RefCell;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use slint_interpreter::{
    CloseRequestResponse, Compiler, ComponentHandle, ComponentInstance, SharedString, Value,
};

/// Small companion window with the reload button and status readout.
const CONTROL_SOURCE: &str = r#"
import { Button, CheckBox } from "std-widgets.slint";

export component ControlWindow inherits Window {
    title: "aura-preview — control";
    min-width: 360px;
    min-height: 150px;
    preferred-width: 460px;
    preferred-height: 190px;

    in property <string> file: "";
    in property <string> status: "";
    in property <bool> status-ok: true;
    in property <bool> watch-enabled: true;
    in-out property <bool> watch: true;

    callback reload-requested();
    callback watch-toggled(bool);
    callback screenshot-requested();

    VerticalLayout {
        padding: 10px;
        spacing: 8px;

        HorizontalLayout {
            spacing: 12px;
            Button {
                text: "Reload";
                clicked => root.reload-requested();
            }
            Button {
                text: "Screenshot";
                clicked => root.screenshot-requested();
            }
            CheckBox {
                text: "Auto-reload on save";
                enabled: root.watch-enabled;
                checked <=> root.watch;
                toggled => root.watch-toggled(root.watch);
            }
        }
        Rectangle {
            background: #111111;
            VerticalLayout {
                padding: 8px;
                spacing: 4px;
                Text {
                    text: root.file;
                    font-size: 13px;
                    overflow: elide;
                    color: #dddddd;
                }
                Text {
                    text: root.status;
                    font-size: 13px;
                    wrap: word-wrap;
                    color: root.status-ok ? #7ec87e : #e07a7a;
                }
            }
        }
    }
}
"#;

/// Auto-reload master switch, toggled from the control window.
static WATCH: AtomicBool = AtomicBool::new(true);

thread_local! {
    static PREVIEW: RefCell<Option<Preview>> = const { RefCell::new(None) };
}

fn with_preview(f: impl FnOnce(&mut Preview)) {
    PREVIEW.with(|p| {
        if let Some(preview) = p.borrow_mut().as_mut() {
            f(preview);
        }
    });
}

struct Preview {
    compiler: Compiler,
    entry: PathBuf,
    component: Option<String>,
    instance: Option<ComponentInstance>,
    control: ComponentInstance,
    reloads: u32,
}

impl Preview {
    /// Recompile the entry file and swap it into the existing window.
    /// Keeps the old UI on compile errors and reports diagnostics instead.
    fn reload(&mut self) {
        let result = spin_on::spin_on(self.compiler.build_from_path(&self.entry));

        let messages: Vec<String> = result.diagnostics().map(|d| d.to_string()).collect();
        let definition = match &self.component {
            Some(name) => result.component(name),
            None => result.components().next(),
        };

        if result.has_errors() || definition.is_none() {
            result.print_diagnostics();
            let status = if messages.is_empty() {
                format!("no exported component in {}", self.entry.display())
            } else {
                messages.join("\n")
            };
            self.set_status(&status, false);
            return;
        }

        let created = match &self.instance {
            Some(old) => definition
                .expect("checked above")
                .create_with_existing_window(old.window()),
            None => definition.expect("checked above").create(),
        };

        match created {
            Ok(instance) => {
                // Closing the plugin window closes the control window too.
                instance.window().on_close_requested(|| {
                    with_preview(|p| {
                        let _ = p.control.hide();
                    });
                    CloseRequestResponse::HideWindow
                });
                // Plugin editors set `version` from CARGO_PKG_VERSION. Preview
                // never runs that code, so mirror it from Cargo.toml when the
                // component exposes the property.
                if let Some(v) = crate_version(&self.entry) {
                    let _ = instance.set_property("version", Value::from(SharedString::from(v)));
                }
                if let Err(e) = instance.show() {
                    self.set_status(&format!("show failed: {e}"), false);
                    return;
                }
                self.instance = Some(instance);
                self.reloads += 1;
                self.set_status(&format!("OK — reload #{}", self.reloads), true);
            }
            Err(e) => self.set_status(&format!("create failed: {e}"), false),
        }
    }

    fn take_screenshot(&self) {
        let Some(instance) = &self.instance else {
            self.set_status("no preview window to capture", false);
            return;
        };
        match instance.window().take_snapshot() {
            Ok(buffer) => {
                let dir = self
                    .entry
                    .parent()
                    .map_or_else(std::env::temp_dir, std::path::PathBuf::from);
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let filename = format!("aura-preview-{timestamp}.png");
                let path = dir.join(filename);
                match save_png(&path, &buffer) {
                    Ok(()) => self.set_status(&format!("saved {}", path.display()), true),
                    Err(e) => self.set_status(&format!("save failed: {e}"), false),
                }
            }
            Err(e) => self.set_status(&format!("snapshot failed: {e}"), false),
        }
    }

    fn set_status(&self, text: &str, ok: bool) {
        let _ = self
            .control
            .set_property("status", Value::from(SharedString::from(text)));
        let _ = self.control.set_property("status-ok", Value::from(ok));
    }
}

fn save_png(
    path: &std::path::Path,
    buffer: &slint_interpreter::SharedPixelBuffer<slint_interpreter::Rgba8Pixel>,
) -> Result<(), Box<dyn std::error::Error>> {
    let width = buffer.width();
    let height = buffer.height();
    let rgba: Vec<u8> = buffer
        .as_slice()
        .iter()
        .flat_map(|p| [p.r, p.g, p.b, p.a])
        .collect();
    let img = image::RgbaImage::from_raw(width, height, rgba).ok_or("invalid image dimensions")?;
    img.save(path)?;
    Ok(())
}

/// Walk up from the entry file and read the crate version.
/// Prefers `[package] version = "…"`. If the crate inherits
/// (`version.workspace = true`), uses `[workspace.package] version`.
fn crate_version(entry: &std::path::Path) -> Option<String> {
    let mut dir = entry.parent()?;
    let mut workspace_fallback = None;
    loop {
        if let Ok(text) = std::fs::read_to_string(dir.join("Cargo.toml")) {
            if let Some(v) = section_quoted_version(&text, "[package]") {
                return Some(v);
            }
            if workspace_fallback.is_none() {
                workspace_fallback = section_quoted_version(&text, "[workspace.package]");
            }
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => return workspace_fallback,
        }
    }
}

/// First `version = "…"` / `'…'` in `section`. Skips `version.workspace` and
/// dependency versions in other tables.
fn section_quoted_version(toml: &str, section: &str) -> Option<String> {
    let mut in_section = false;
    for line in toml.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if t.starts_with('[') {
            in_section = t == section;
            continue;
        }
        if !in_section {
            continue;
        }
        let Some(rest) = t.strip_prefix("version") else {
            continue;
        };
        let rest = rest.trim_start();
        if rest.starts_with('.') {
            continue;
        }
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        return quoted_string(rest);
    }
    None
}

fn quoted_string(s: &str) -> Option<String> {
    let s = s.trim();
    let q = s.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let v = s.get(1..)?.split(q).next()?;
    if v.is_empty() {
        None
    } else {
        Some(v.to_string())
    }
}

struct Args {
    entry: PathBuf,
    component: Option<String>,
    watch: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut entry = None;
    let mut component = None;
    let mut watch = true;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                println!(
                    "aura-preview — hot-reload preview for AURA plugin .slint UIs\n\
                     \n\
                     Usage:\n  \
                     aura-preview [path/to/main.slint] [--component <Name>] [--no-watch]\n\
                     \n\
                     Defaults to ui/main.slint. The control window has a Reload\n\
                     button; with auto-reload (default) saving a .slint/.ttf file\n\
                     in the UI directory reloads automatically."
                );
                std::process::exit(0);
            }
            "--no-watch" => watch = false,
            "--component" => {
                component = Some(it.next().ok_or("--component needs a name")?);
            }
            other if other.starts_with('-') => return Err(format!("unknown flag: {other}")),
            _ => {
                if entry.is_some() {
                    return Err(format!("multiple paths given (already {entry:?})"));
                }
                entry = Some(PathBuf::from(&arg));
            }
        }
    }
    Ok(Args {
        entry: entry.unwrap_or_else(|| PathBuf::from("ui/main.slint")),
        component,
        watch,
    })
}

/// Watch the UI directory and trigger a debounced reload on `.slint`/`.ttf`
/// changes. Runs the reload on the Slint event-loop thread.
fn spawn_watcher(dir: &std::path::Path) -> Result<notify::RecommendedWatcher, String> {
    use notify::{EventKind, RecursiveMode, Watcher};

    let (tx, rx) = mpsc::channel::<()>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(event) = res else { return };
        if !matches!(
            event.kind,
            EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
        ) {
            return;
        }
        let relevant = event.paths.iter().any(|p| {
            matches!(
                p.extension().and_then(|e| e.to_str()),
                Some("slint" | "ttf")
            )
        });
        if relevant {
            let _ = tx.send(());
        }
    })
    .map_err(|e| format!("watcher init: {e}"))?;
    watcher
        .watch(dir, RecursiveMode::Recursive)
        .map_err(|e| format!("watch {}: {e}", dir.display()))?;

    std::thread::spawn(move || {
        while rx.recv().is_ok() {
            // Debounce: drain bursts (editors saving via rename fire several).
            while rx.recv_timeout(Duration::from_millis(300)).is_ok() {}
            if WATCH.load(Ordering::Relaxed) {
                let _ = slint_interpreter::invoke_from_event_loop(|| {
                    with_preview(Preview::reload);
                });
            }
        }
    });

    Ok(watcher)
}

/// Build the small control window: compile the embedded source, wire the
/// reload button / watch toggle, and set up the close hooks.
fn create_control(
    compiler: &Compiler,
    entry: &std::path::Path,
    watch: bool,
) -> Result<ComponentInstance, String> {
    let result = spin_on::spin_on(
        compiler.build_from_source(CONTROL_SOURCE.into(), PathBuf::from("control.slint")),
    );
    result.print_diagnostics();
    let definition = result
        .component("ControlWindow")
        .ok_or("internal error: control window failed to compile")?;
    let control = definition
        .create()
        .map_err(|e| format!("creating control window: {e}"))?;

    let _ = control.set_property(
        "file",
        Value::from(SharedString::from(entry.display().to_string())),
    );
    let _ = control.set_property("watch", Value::from(watch));
    let _ = control.set_property("watch-enabled", Value::from(watch));

    control
        .set_callback("reload-requested", |_| {
            with_preview(Preview::reload);
            Value::Void
        })
        .expect("callback exists in CONTROL_SOURCE");
    control
        .set_callback("screenshot-requested", |_| {
            with_preview(|p| p.take_screenshot());
            Value::Void
        })
        .expect("callback exists in CONTROL_SOURCE");
    control
        .set_callback("watch-toggled", |args| {
            if let Some(Value::Bool(on)) = args.first() {
                WATCH.store(*on, Ordering::Relaxed);
            }
            Value::Void
        })
        .expect("callback exists in CONTROL_SOURCE");

    // Closing the control window closes the plugin window too.
    control.window().on_close_requested(|| {
        with_preview(|p| {
            if let Some(instance) = &p.instance {
                let _ = instance.hide();
            }
        });
        CloseRequestResponse::HideWindow
    });

    Ok(control)
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}\ntry --help");
            return ExitCode::FAILURE;
        }
    };

    let entry = match args.entry.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}: {e}", args.entry.display());
            return ExitCode::FAILURE;
        }
    };

    // Same assets the real build uses; cached in temp, rewritten only on change.
    let cache_dir = std::env::temp_dir().join("aura-preview");
    let assets = match aura_build::materialize_assets(&cache_dir) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("materializing @aura assets: {e}");
            return ExitCode::FAILURE;
        }
    };

    let mut compiler = Compiler::new();
    compiler.set_library_paths(assets.library_paths);
    let mut includes = assets.include_paths;
    if let Some(dir) = entry.parent() {
        includes.push(dir.to_path_buf());
    }
    compiler.set_include_paths(includes);
    compiler.set_style(assets.style);

    // Control window (embedded source — must always compile).
    let control = match create_control(&compiler, &entry, args.watch) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    WATCH.store(args.watch, Ordering::Relaxed);
    PREVIEW.with(|p| {
        *p.borrow_mut() = Some(Preview {
            compiler,
            entry: entry.clone(),
            component: args.component,
            instance: None,
            control,
            reloads: 0,
        });
    });

    // Keep the watcher alive for the whole event loop.
    let _watcher = if args.watch {
        let dir = entry.parent().unwrap_or(&entry);
        match spawn_watcher(dir) {
            Ok(w) => Some(w),
            Err(e) => {
                eprintln!("auto-reload disabled: {e}");
                None
            }
        }
    } else {
        None
    };

    with_preview(|p| {
        p.reload();
        if let Err(e) = p.control.show() {
            eprintln!("showing control window: {e}");
        }
    });

    match slint_interpreter::run_event_loop() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("event loop: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_toml_version_parse() {
        assert_eq!(
            section_quoted_version(
                "[package]\nname = \"aether\"\nversion = \"1.4.2\"\n\n[dependencies]\naura = { version = \"0.10.0\" }\n",
                "[package]"
            )
            .as_deref(),
            Some("1.4.2")
        );
        assert_eq!(
            section_quoted_version("[package]\nversion.workspace = true\n", "[package]"),
            None
        );
        assert_eq!(
            section_quoted_version(
                "[workspace.package]\nversion = \"0.10.0\"\n",
                "[workspace.package]"
            )
            .as_deref(),
            Some("0.10.0")
        );
        assert_eq!(
            section_quoted_version("[package]\nversion = '1.11.2' # comment\n", "[package]")
                .as_deref(),
            Some("1.11.2")
        );

        let smoke = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/smoke-gain/ui/main.slint");
        assert_eq!(
            crate_version(&smoke).as_deref(),
            Some(env!("CARGO_PKG_VERSION"))
        );
    }
}
