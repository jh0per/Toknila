//! What a run's agent may read, through the real `tok` binary and daemon with the `fake-agent`
//! as the planner's command (no real agent is launched): the fake agent judges a read inside the
//! repository and reads outside it by the settings file the runner wrote, the allow rules and the
//! deny rules apart. The profile asks for a bare `Read`, `Grep` and `Glob`, as the built-in one
//! does; none of them may reach the file.

#![cfg(unix)]

mod common;

use std::time::{Duration, Instant};

use common::{Tok, repo};

const WAIT: Duration = Duration::from_secs(60);

#[test]
fn the_agent_may_read_inside_the_repository_and_not_outside_by_the_allow_rules_alone() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tok = Tok::in_dir(dir.path());
    let repo = repo(dir.path());
    let outside = dir.path().join("outside.txt");
    std::fs::write(&outside, "x").expect("write");
    let outside = outside.display().to_string();
    tok.profile(
        "reader",
        &[
            "readcheck",
            "--settings",
            "{cwd}/README.md",
            "readcheck",
            "--settings",
            "{cwd}/src/deep/file.rs",
            "readcheck",
            "--settings",
            &outside,
            "readcheck",
            "--settings",
            "{cwd}-sibling/x",
            "readcheck",
            "--settings",
            "/etc/passwd",
            "print",
            "reads-done",
            "hold",
        ],
        "role = 'planner'\nstartup_secs = 0\n[hooks]\nenabled = false\nsettings_flag = '--settings'\n\
         [permissions]\nallow = ['Read', 'Grep', 'Glob']\n",
    );
    tok.run(&["project", "new", "DEMO", "--repo", &repo.to_string_lossy()]);
    tok.run(&["planner", "start", "DEMO", "--profile", "reader"]);
    let until = Instant::now() + WAIT;
    let screen = loop {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_tok"))
            .args(["planner", "logs", "DEMO"])
            .env("TOKNILA_HOME", &tok.home)
            .output()
            .expect("run tok planner logs");
        let text = String::from_utf8_lossy(&out.stdout).replace(['\r', '\n'], "");
        if text.contains("reads-done") {
            break text;
        }
        assert!(Instant::now() < until, "timed out: {text}");
        std::thread::sleep(Duration::from_millis(100));
    };
    // The verdict printed for the line that ends in `tail`.
    let verdict = |tail: &str| {
        let end = screen
            .find(tail)
            .unwrap_or_else(|| panic!("no line for {tail}: {screen}"));
        let start = screen[..end].rfind("read allow=").expect("a verdict");
        screen[start..end].to_owned()
    };
    // The repository's own files: allowed by the allow rules, denied by none.
    assert!(verdict("/README.md").starts_with("read allow=yes deny=no "));
    assert!(verdict("/src/deep/file.rs").starts_with("read allow=yes deny=no "));
    // Outside it the allow rules alone say no, and nothing is denied either: the deny list is not
    // what keeps these out.
    assert!(verdict(&outside).starts_with("read allow=no deny=no "));
    assert!(verdict("-sibling/x").starts_with("read allow=no deny=no "));
    assert!(verdict("/etc/passwd").starts_with("read allow=no deny=no "));
    tok.run(&["planner", "stop", "DEMO"]);
}
