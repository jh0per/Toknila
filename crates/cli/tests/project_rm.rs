//! P1-29: `tok project rm` through the real `tok` binary and daemon: deleting the records keeps
//! the repository folder, and moving the folder to the Trash is refused unless the person types
//! the key at a terminal (or passes `--yes`, which no test does: the real Trash is not touched).

mod common;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use common::Tok;

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?}: {out:?}");
}

/// A repository whose only branch is `develop`, with one commit.
fn repo(dir: &Path) -> PathBuf {
    let repo = dir.join("repo");
    std::fs::create_dir_all(&repo).expect("mkdir");
    git(&repo, &["init", "--quiet"]);
    git(&repo, &["symbolic-ref", "HEAD", "refs/heads/develop"]);
    std::fs::write(repo.join("README.md"), "hi\n").expect("write");
    git(&repo, &["add", "README.md"]);
    git(&repo, &["commit", "--quiet", "-m", "init"]);
    repo
}

/// Project DEMO on a fresh repository, with chain `auth` and one card that has not started.
fn planned(dir: &Path) -> (Tok, PathBuf) {
    let tok = Tok {
        home: dir.join("home"),
    };
    let repo = repo(dir);
    let path = repo.to_str().expect("utf-8 path");
    tok.run(&["project", "new", "DEMO", "--repo", path]);
    tok.run(&[
        "chain",
        "new",
        "auth",
        "--project",
        "DEMO",
        "--no-autostart",
    ]);
    tok.run(&["ticket", "new", "auth", "Login"]);
    (tok, repo)
}

#[test]
fn records_only_deletes_the_project_and_keeps_its_folder() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (tok, repo) = planned(dir.path());
    let out = tok.run(&["project", "rm", "DEMO"]);
    assert_eq!(out["project"]["key"], "DEMO");
    assert_eq!(out["chains"], serde_json::json!(["auth"]));
    assert_eq!(out["tickets"], 1);
    assert_eq!(out["folder"], "kept");
    assert_eq!(out["warnings"], serde_json::json!([]));
    assert_eq!(tok.run(&["project", "ls"]), serde_json::json!([]));
    assert!(repo.join("README.md").is_file());
    let again = tok.fails(&["project", "rm", "DEMO"]);
    assert!(again.contains("project DEMO not found"), "{again}");
}

#[test]
fn trashing_the_folder_is_refused_without_a_terminal_to_type_the_key() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (tok, repo) = planned(dir.path());
    // The key arrives on stdin, but nobody is at a terminal: refused before anything changes.
    let mut child = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["project", "rm", "DEMO", "--trash-folder"])
        .env("TOKNILA_HOME", &tok.home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run tok");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"DEMO\n")
        .expect("type");
    let out = child.wait_with_output().expect("tok ends");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("needs a confirmation and stdin is not a terminal; pass --yes"),
        "{stderr}"
    );
    let projects = tok.run(&["project", "ls"]);
    assert_eq!(projects[0]["key"], "DEMO");
    assert_eq!(tok.status("DEMO-1"), "ready");
    assert!(repo.join("README.md").is_file());
}
