//! P1-05: `tok profile ls` through the real binary and daemon: the built-in profiles, the files
//! of the profiles folder, and an invalid file named with its line and column.

mod common;

use std::process::Command;

use common::{Tok, snapshot};
use serde_json::json;

#[test]
fn profile_ls_lists_built_ins_and_files_and_names_the_invalid_one() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tok = Tok::in_dir(dir.path());
    let folder = tok.home.join("profiles");
    std::fs::create_dir_all(&folder).expect("mkdir");
    std::fs::write(
        folder.join("my-agent.toml"),
        "role = \"reviewer\"\ncommand = [\"my-agent\", \"--headless\"]\n",
    )
    .expect("write profile");
    std::fs::write(
        folder.join("broken.toml"),
        "command = [\"a\"]\n\n[nudge]\nidle_sec = 5\n",
    )
    .expect("write broken profile");

    let mut list = tok.run(&["profile", "ls"]);
    assert_eq!(
        list["folder"],
        json!(folder.to_string_lossy()),
        "the folder is reported"
    );
    list["folder"] = json!("<home>/profiles");
    snapshot("profile_ls", list);

    let out = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["profile", "ls"])
        .env("TOKNILA_HOME", &tok.home)
        .output()
        .expect("run tok");
    assert!(out.status.success(), "{out:?}");
    let table = String::from_utf8_lossy(&out.stdout);
    for want in [
        "sim",
        "claude-code",
        "claude",
        "my-agent",
        "reviewer",
        "built-in",
    ] {
        assert!(table.contains(want), "{want:?} missing from:\n{table}");
    }
    assert!(!table.contains("broken"), "{table}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    let at = folder.join("broken.toml");
    assert!(
        stderr.starts_with(&format!("{}:4:1: ", at.display())) && stderr.contains("idle_sec"),
        "file, line and column first, then the reason: {stderr}"
    );
}
