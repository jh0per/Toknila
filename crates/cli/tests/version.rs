//! Runs the real binaries, so a broken `[[bin]]` entry or a renamed alias fails here.

use std::process::Command;

fn version_output(bin_path: &str) -> String {
    let out = Command::new(bin_path)
        .arg("--version")
        .output()
        .expect("binary should start");
    assert!(
        out.status.success(),
        "{bin_path} --version exited with {}",
        out.status
    );
    String::from_utf8(out.stdout).expect("utf-8 output")
}

#[test]
fn toknila_prints_version() {
    let text = version_output(env!("CARGO_BIN_EXE_toknila"));
    assert_eq!(text.trim(), format!("toknila {}", toknila_core::VERSION));
}

#[test]
fn tok_alias_prints_same_version() {
    let text = version_output(env!("CARGO_BIN_EXE_tok"));
    assert_eq!(text.trim(), format!("toknila {}", toknila_core::VERSION));
}
