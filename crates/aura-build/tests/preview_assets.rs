//! The interpreter path used by `aura-preview` must compile a plugin UI
//! exactly like the build-script path in `compile()` does.

use std::path::PathBuf;

#[test]
fn materialized_assets_compile_plugin_ui() {
    let tmp = std::env::temp_dir().join("aura-build-test-assets");
    let assets = aura_build::materialize_assets(&tmp).expect("materialize assets");

    let mut compiler = slint_interpreter::Compiler::new();
    compiler.set_library_paths(assets.library_paths);
    compiler.set_include_paths(assets.include_paths);
    compiler.set_style(assets.style);

    let entry =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/smoke-gain/ui/main.slint");
    let result = spin_on::spin_on(compiler.build_from_path(&entry));

    for d in result.diagnostics() {
        eprintln!("{d}");
    }
    assert!(!result.has_errors(), "compiling {}", entry.display());
    assert!(
        result.component("AppWindow").is_some(),
        "AppWindow must be exported"
    );
}
