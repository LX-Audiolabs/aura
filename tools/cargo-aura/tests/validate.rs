use std::fs;
use std::process::Command;

#[test]
fn validate_preserves_arguments_output_and_exit_status() {
    let dir = std::env::temp_dir().join(format!("aura validator test {}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("validator.rs");
    let binary = dir.join(format!("clap-validator{}", std::env::consts::EXE_SUFFIX));
    fs::write(
        &source,
        r#"
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["validate", "--help"] {
        println!("validator help");
        return;
    }
    if args == ["validate", "--windows-exit"] {
        std::process::exit(513);
    }
    assert_eq!(args, ["validate", "two plugins.clap", "--json"]);
    println!("{{\"valid\":false}}");
    eprintln!("validator stderr");
    std::process::exit(17);
}
"#,
    )
    .unwrap();
    assert!(
        Command::new("rustc")
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .status()
            .unwrap()
            .success()
    );

    let cli = env!("CARGO_BIN_EXE_cargo-aura");
    let output = Command::new(cli)
        .args(["aura", "validate", "two plugins.clap", "--json"])
        .env("PATH", &dir)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(17));
    assert_eq!(output.stdout, b"{\"valid\":false}\n");
    assert_eq!(output.stderr, b"validator stderr\n");

    let help = Command::new(cli)
        .args(["validate", "--help"])
        .env("PATH", &dir)
        .output()
        .unwrap();
    assert!(help.status.success());
    assert_eq!(help.stdout, b"validator help\n");

    #[cfg(windows)]
    {
        let status = Command::new(cli)
            .args(["validate", "--windows-exit"])
            .env("PATH", &dir)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(513));
    }

    fs::remove_file(&binary).unwrap();
    let missing = Command::new(cli)
        .args(["validate", "plugin.clap"])
        .env("PATH", &dir)
        .output()
        .unwrap();
    assert!(!missing.status.success());
    assert_eq!(missing.stdout, b"");
    assert!(
        String::from_utf8(missing.stderr)
            .unwrap()
            .contains("clap-validator")
    );
    fs::remove_dir_all(&dir).unwrap();
}
