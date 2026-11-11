//! A card returned from Review restarts the worker, and that run's kickoff and instructions file
//! carry the Return (through the real `tok` binary, a daemon and the `fake-agent` on a
//! pseudo-terminal, never a real agent). A hand-in that carries no new commit is refused.

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::{Tok, repo};

const WAIT: Duration = Duration::from_secs(30);

fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let until = Instant::now() + WAIT;
    while !ok() {
        assert!(Instant::now() < until, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn head(ship: &Path) -> String {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(ship)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("git runs");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

fn commit(ship: &Path, file: &str) {
    std::fs::write(ship.join(file), format!("{file}\n")).expect("write");
    common::git(ship, &["add", file]);
    common::git(ship, &["commit", "--quiet", "-m", file]);
}

/// The agent's screen with the terminal's line wrapping taken out.
fn screen(tok: &Tok) -> String {
    tok.logs("DEMO-1").replace(['\r', '\n'], "")
}

#[test]
fn a_return_is_in_the_new_runs_kickoff_and_a_hand_in_at_the_same_commit_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tok = Tok::in_dir(dir.path());
    // It prints its first message and its instructions, then stays.
    tok.profile(
        "returner",
        &[
            "print",
            "{kickoff}",
            "print",
            "begin-instructions",
            "flagfile",
            "--instr",
            "print",
            "end-instructions",
            "sleep",
            "30000",
        ],
        "instructions_flag = \"--instr\"\n",
    );
    let repo = repo(dir.path());
    tok.run(&[
        "project",
        "new",
        "DEMO",
        "--repo",
        &repo.to_string_lossy(),
        "--profile",
        "returner",
    ]);
    tok.run(&["chain", "new", "demo"]);
    tok.run(&["ticket", "new", "demo", "Alpha"]);
    wait_until("the first run's instructions", || {
        tok.logs("DEMO-1").contains("end-instructions")
    });
    let ship = PathBuf::from(
        tok.run(&["show", "DEMO-1"])["ship"]["path"]
            .as_str()
            .expect("ship"),
    );
    commit(&ship, "a.txt");
    let first = head(&ship);
    tok.run(&["sim", "DEMO-1", "finish", "done"]);
    assert_eq!(tok.status("DEMO-1"), "review");
    let log_before = tok.logs("DEMO-1").len();
    assert!(!screen(&tok).contains("This card was returned"));

    tok.wait_for_person_turn("DEMO-1");
    tok.run(&["return", "DEMO-1", "Rename the flag"]);
    wait_until("the new run to print the Return", || {
        let log = tok.logs("DEMO-1");
        log.len() > log_before && log.contains("file:## This card was returned")
    });
    let seen = screen(&tok);
    let short = &first[..7];
    // Round 1 is the hand-in's own check (every card with a ship is brought up to its base and
    // checked); the Return is round 2.
    assert!(
        seen.contains("This card was returned: round 2, by the person"),
        "{seen}"
    );
    assert!(
        seen.contains(&format!("sent back your hand-in {short}")),
        "{seen}"
    );
    assert!(seen.contains("\"Rename the flag\""), "{seen}");
    assert!(seen.contains("file:> Rename the flag"), "{seen}");
    assert!(
        seen.contains(&format!("after your hand-in at commit {short}")),
        "{seen}"
    );
    // The run read it: it is delivered, so the Stop hook has nothing to type.
    wait_until("the Return to be delivered", || {
        tok.run(&["show", "DEMO-1"])["comments"]
            .as_array()
            .is_some_and(|c| {
                c.iter()
                    .any(|c| c["kind"] == "instruction" && !c["delivered_at"].is_null())
            })
    });
    assert_eq!(tok.status("DEMO-1"), "running");

    // The same commit again is refused, quoting the Return; the card stays Running.
    let refused = tok.fails(&["sim", "DEMO-1", "finish", "done again"]);
    assert!(refused.contains("must carry new commits"), "{refused}");
    assert!(refused.contains("Rename the flag"), "{refused}");
    assert!(refused.contains(short), "{refused}");
    assert_eq!(tok.status("DEMO-1"), "running");

    // A new commit is accepted.
    commit(&ship, "b.txt");
    tok.run(&["sim", "DEMO-1", "finish", "done again"]);
    assert_eq!(tok.status("DEMO-1"), "review");

    // The hand-in after the Return carries the Since the Return block, and `tok show` prints it.
    let shown = tok.text(&["show", "DEMO-1"]);
    let block = shown
        .split("Since the Return")
        .nth(1)
        .unwrap_or_else(|| panic!("no Since the Return block: {shown}"));
    assert!(block.starts_with(" (round 2, "), "{shown}");
    assert!(block.contains("  - b.txt\n"), "{shown}");
    assert!(block.contains("Changed files: b.txt"), "{shown}");
    assert!(!block.contains("a.txt"), "{shown}");

    // A commit made after the hand-in is refused at Accept, named, with the two ways out.
    tok.wait_for_person_turn("DEMO-1");
    commit(&ship, "c.txt");
    let refused = tok.fails(&["accept", "DEMO-1"]);
    assert!(refused.contains("beyond the hand-in"), "{refused}");
    assert!(refused.contains("c.txt"), "{refused}");
    assert!(refused.contains("Return the card"), "{refused}");
    assert!(refused.contains("wait for a new hand-in"), "{refused}");
    assert_eq!(tok.status("DEMO-1"), "review");
    tok.run(&["cancel", "DEMO-1", "done"]);
}
