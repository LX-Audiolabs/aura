//! Build-script helper for AURA plugins with a Slint GUI.
//!
//! Wraps [`slint_build::compile_with_config`] and pre-fills the AURA bits —
//! the `@aura` widget library path and the include path that lets `.slint`
//! files `import "JetBrainsMono-Regular.ttf";` etc.
//!
//! ```rust,ignore
//! // build.rs
//! fn main() {
//!     aura_build::compile("ui/main.slint").unwrap();
//! }
//! ```
//!
//! In `.slint`:
//! ```text
//! import { Knob, Meter, AuraTheme } from "@aura";
//! import "JetBrainsMono-Regular.ttf";
//! // colors: AuraTheme.surface / .primary (Material 3 dark tokens)
//! ```

use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// Filenames under `OUT_DIR/fonts/` — contract for plugin `.slint` imports.
const FONT_SOURCES: &[(&str, &[u8])] = &[
    (
        "JetBrainsMono-Regular.ttf",
        include_bytes!("../fonts/JetBrainsMono-Regular.ttf"),
    ),
    (
        "JetBrainsMono-Bold.ttf",
        include_bytes!("../fonts/JetBrainsMono-Bold.ttf"),
    ),
    (
        "NotoSans-Regular.ttf",
        include_bytes!("../fonts/NotoSans-Regular.ttf"),
    ),
    (
        "NotoSans-Bold.ttf",
        include_bytes!("../fonts/NotoSans-Bold.ttf"),
    ),
    (
        "Selawik-Regular.ttf",
        include_bytes!("../fonts/Selawik-Regular.ttf"),
    ),
    (
        "Selawik-Bold.ttf",
        include_bytes!("../fonts/Selawik-Bold.ttf"),
    ),
];

/// `@aura` library import name (`import { Knob } from "@aura";`).
const LIBRARY_NAME: &str = "aura";

const WIDGET_SOURCES: &[(&str, &str)] = &[
    ("widgets.slint", include_str!("../ui/widgets.slint")),
    ("theme.slint", include_str!("../ui/theme.slint")),
    ("knob.slint", include_str!("../ui/knob.slint")),
    ("meter.slint", include_str!("../ui/meter.slint")),
    ("dropdown.slint", include_str!("../ui/dropdown.slint")),
    ("slider.slint", include_str!("../ui/slider.slint")),
    ("toggle.slint", include_str!("../ui/toggle.slint")),
    ("xy_pad.slint", include_str!("../ui/xy_pad.slint")),
];

/// Where the bundled widgets and fonts were written, and how the Slint
/// compiler must be configured to resolve `@aura` and font imports.
///
/// Returned by [`materialize_assets`]; usable outside build scripts (e.g.
/// the `aura-preview` hot-reload viewer configures `slint_interpreter`
/// with exactly these paths).
#[derive(Debug, Clone)]
pub struct AssetPaths {
    /// `"aura"` → path of the materialized `widgets.slint`.
    pub library_paths: std::collections::HashMap<String, PathBuf>,
    /// Directory containing the bundled `.ttf` files.
    pub include_paths: Vec<PathBuf>,
    /// Pinned widget style (`"fluent"`).
    pub style: String,
}

/// Write the bundled `@aura` widgets and fonts into `dir/aura-build/{ui,fonts}`
/// and return the compiler configuration pointing at them.
///
/// Unlike [`compile`] this does not need `OUT_DIR` and emits no cargo
/// directives, so it works in any process.
///
/// # Errors
///
/// [`CompileError`] if the assets cannot be written.
pub fn materialize_assets(dir: &Path) -> Result<AssetPaths, CompileError> {
    let ui_dir = materialize_widgets(dir)?;
    let font_dir = materialize_font(dir)?;

    let mut library_paths = std::collections::HashMap::new();
    library_paths.insert(LIBRARY_NAME.to_string(), ui_dir.join("widgets.slint"));

    Ok(AssetPaths {
        library_paths,
        include_paths: vec![font_dir],
        // Pin Fluent so ComboBox / std-widgets look the same on every OS.
        style: "fluent".to_string(),
    })
}

/// Compile `slint_entry` (relative to the caller's `CARGO_MANIFEST_DIR`)
/// with the AURA widget library and font include path.
///
/// # Errors
///
/// [`CompileError`] if `OUT_DIR` is missing, assets cannot be written, or
/// `slint-build` rejects the input.
pub fn compile(slint_entry: impl AsRef<Path>) -> Result<(), CompileError> {
    let out_dir = std::env::var_os("OUT_DIR")
        .map(PathBuf::from)
        .ok_or(CompileError::NoOutDir)?;

    // Rerun if crate fonts change (when building from git path).
    println!("cargo:rerun-if-changed=fonts");
    println!("cargo:rerun-if-changed=ui");

    let assets = materialize_assets(&out_dir)?;

    let config = slint_build::CompilerConfiguration::new()
        .with_library_paths(assets.library_paths)
        .with_include_paths(assets.include_paths)
        .with_style(assets.style);

    slint_build::compile_with_config(slint_entry, config)
        .map_err(|e| CompileError::Slint(format!("{e}")))?;

    Ok(())
}

/// Errors from [`compile`].
#[derive(Debug)]
pub enum CompileError {
    NoOutDir,
    Io(std::io::Error),
    Slint(String),
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoOutDir => {
                f.write_str("OUT_DIR not set — call aura_build::compile from a build script")
            }
            Self::Io(e) => write!(f, "writing bundled assets to OUT_DIR: {e}"),
            Self::Slint(e) => write!(f, "slint compile failed: {e}"),
        }
    }
}

impl Error for CompileError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for CompileError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

fn materialize_widgets(out_dir: &Path) -> Result<PathBuf, CompileError> {
    let ui_dir = out_dir.join("aura-build/ui");
    fs::create_dir_all(&ui_dir)?;
    for (name, source) in WIDGET_SOURCES {
        write_if_changed(&ui_dir.join(name), source.as_bytes())?;
    }
    Ok(ui_dir)
}

fn materialize_font(out_dir: &Path) -> Result<PathBuf, CompileError> {
    let font_dir = out_dir.join("aura-build/fonts");
    fs::create_dir_all(&font_dir)?;
    for (name, bytes) in FONT_SOURCES {
        write_if_changed(&font_dir.join(name), bytes)?;
    }
    Ok(font_dir)
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Ok(existing) = fs::read(path)
        && existing == bytes
    {
        return Ok(());
    }
    fs::write(path, bytes)
}
