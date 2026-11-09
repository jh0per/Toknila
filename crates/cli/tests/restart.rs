//! A run that replaces another on the same ship, through the real `tok` binary, a daemon and the
//! workspace's `fake-agent`: an agent killed from outside is blocked as terminated, and the run
//! that Unblock starts is told the instructions its predecessor was given.

mod common;

use std::time::{Duration, Instant};

use common::{Tok, kill_hard, repo};

const WAIT: Duration = Duration::from_secs(30);

fn wait_until(tok: &Tok, what: &str, mut ok: impl FnMut() -> bool) {
    let until = Instant::now() + WAIT;
    while !ok() {
        assert!(
            Instant::now() < until,
            "timed out waiting for {what}\n{}",
            tok.logs("DEMO-1")
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn an_agent_killed_from_outside_is_terminated_and_its_replacement_sees_the_earlier_instruction() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tok = Tok::in_dir(dir.path());
    let repo = repo(dir.path());
    tok.profile(
        "worker",
        &[
            "print", "ready", "pid", "flagfile", "--instr", "sleep", "30000",
        ],
        "instructions_flag = \"--instr\"\n",
    );
    tok.run(&[
        "project",
        "new",
        "DEMO",
        "--repo",
        &repo.to_string_lossy(),
        "--profile",
        "worker",
    ]);
    tok.run(&["chain", "new", "demo"]);
    tok.run(&["ticket", "new", "demo", "Alpha"]);
    wait_until(&tok, "the first run to print its instructions", || {
        tok.logs("DEMO-1").contains("file:## Task")
    });
    assert!(
        !tok.logs("DEMO-1").contains("given earlier"),
        "the first run has nothing earlier"
    );

    tok.run(&["comment", "DEMO-1", "use the Edit tool"]);
    wait_until(&tok, "the instruction to be delivered", || {
        let shown = tok.run(&["show", "DEMO-1"]);
        let comments = shown["comments"].as_array().cloned().unwrap_or_default();
        comments.len() == 1 && !comments[0]["delivered_at"].is_null()
    });

    let log = tok.logs("DEMO-1");
    let pid: u32 = log
        .lines()
        .find_map(|l| l.trim().strip_prefix("pid="))
        .and_then(|p| p.parse().ok())
        .expect("the agent printed its pid");
    kill_hard(pid);
    wait_until(&tok, "the card to block", || {
        tok.status("DEMO-1") == "blocked"
    });
    let reason = tok.run(&["show", "DEMO-1"])["ticket"]["status_reason"].clone();
    let shown = tok.text(&["show", "DEMO-1"]);
    if cfg!(unix) {
        // A signal death is told apart from a crash; Windows has no signal to read, so a killed
        // agent there is a crash with its exit code.
        assert_eq!(reason["reason"]["kind"], "terminated", "{reason}");
        assert!(
            shown.contains("terminated from outside") && shown.contains("unblock starts it again"),
            "{shown}"
        );
    } else {
        assert_eq!(reason["reason"]["kind"], "crashed", "{reason}");
    }

    tok.run(&["unblock", "DEMO-1"]);
    wait_until(&tok, "the second run to print its instructions", || {
        let out = tok.logs("DEMO-1");
        out.contains("file:## Instructions you were given earlier")
            && out.contains("use the Edit tool")
    });
    assert_eq!(tok.status("DEMO-1"), "running");
    tok.run(&["cancel", "DEMO-1", "done"]);
}
