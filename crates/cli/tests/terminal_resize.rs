//! TOKNILA-162: what the page does when it opens a card's terminal while the agent prints, in a
//! real daemon started as `tok` starts it (detached, with no console of its own on Windows): its
//! terminal answers the cursor query it finds in the scrollback (ConPTY's `ESC [ 6 n`, which the
//! daemon already answered) by typing `ESC [ 1 ; 1 R`, and it sends the panel's size. The run log
//! keeps every line the agent printed.
//!
//! On Windows a ConPTY shrunk early in the output drops the first lines (the daemon's
//! `terminal_lines` probe), so the runner holds a smaller size until the output is quiet. Sent
//! at once, this test's way lost nothing in 12 of 12 runs in a detached daemon on GitHub's Windows
//! runner (CI runs 44338573473 and 44348226497); the race sits a few milliseconds after the cursor
//! answer, earlier than a client reaches it here, so this is the end-to-end check of what holds.

mod common;
/// The one lock of the tests that open ptys, shared with the daemon's `runner.rs`.
#[path = "../../daemon/tests/support/mod.rs"]
mod pty_lock;

use std::time::{Duration, Instant};

use common::{Tok, repo};

/// A ceiling, not a delay.
const WAIT: Duration = Duration::from_secs(60);
/// Runs of the agent.
const RUNS: usize = 3;
const LINES: u32 = 80;

/// The numbered lines of 1..=80 that `text` lacks ("scroll line 1" is not "scroll line 10").
fn missing(text: &str) -> Vec<u32> {
    (1..=LINES)
        .filter(|i| {
            let line = format!("scroll line {i}");
            !text
                .match_indices(&line)
                .any(|(at, m)| !text[at + m.len()..].starts_with(|c: char| c.is_ascii_digit()))
        })
        .collect()
}

fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let until = Instant::now() + WAIT;
    while !ok() {
        assert!(Instant::now() < until, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_client_that_opens_the_terminal_while_the_agent_prints_costs_no_line_of_the_run_log() {
    let _turn = pty_lock::hold();
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    let repo = repo(dir.path());
    let mut steps = vec!["sleep".to_owned(), "0".to_owned()];
    for i in 1..=LINES {
        steps.extend(["print".to_owned(), format!("scroll line {i}")]);
        steps.extend(["sleep".to_owned(), "2".to_owned()]);
    }
    steps.extend(["print".to_owned(), "printed all".to_owned()]);
    steps.extend(["hold".to_owned()]);
    let steps: Vec<&str> = steps.iter().map(String::as_str).collect();
    tok.profile("printer", &steps, "");
    let repo = repo.to_string_lossy();
    let project = ["project", "new", "DEMO", "--repo", &repo];
    tok.run(&[&project[..], &["--profile", "printer"]].concat());
    tok.run(&["chain", "new", "demo", "--no-autostart"]);
    let small = toknila_api_types::TerminalSize {
        cols: 152,
        rows: 14,
    };
    let mut lost = Vec::new();
    for n in 1..=RUNS {
        let key = format!("DEMO-{n}");
        tok.run(&["ticket", "new", "demo", &format!("Run {n}")]);
        tok.run(&["start", &key]);
        wait_until("the first line", || tok.logs(&key).contains("scroll line"));
        let client = tok.client();
        client.runner_input(&key, b"\x1b[1;1R").expect("answer");
        client.runner_size(&key, &small).expect("resize");
        wait_until("the last line", || tok.logs(&key).contains("printed all"));
        lost.push(missing(&tok.logs(&key)));
        tok.run(&["cancel", &key, "done"]);
    }
    assert!(
        lost.iter().all(Vec::is_empty),
        "lines missing from the run log, per run: {lost:?}"
    );
}
