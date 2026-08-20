// Fallback for rust-analyzer when i-slint-compiler build.rs env is missing.
fn widget_library() -> &'static [(&'static str, &'static BuiltinDirectory<'static>)] {
    &[]
}
