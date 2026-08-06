fn main() {
    println!("cargo:rerun-if-changed=src/gui/ui.slint");
    slint_build::compile("src/gui/ui.slint").unwrap();
}
