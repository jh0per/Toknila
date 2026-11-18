//! TOKNILA-162: what the page does when it opens a card's terminal while the agent prints, in a
//! real daemon started as `tok` starts it (detached, with no console of its own on Windows): it
//! sends the panel's size, and its terminal answers the cursor query it finds in the scrollback
//! (ConPTY's `ESC [ 6 n`, which the daemon already answered) by typing `ESC [ 1 ; 1 R` into the
//! agent. The run log must keep every line the agent printed.
//!
//! The terminal-copy spec's run log lacked lines 1-12 and 1-8 in two of six Windows runs (CI run
//! 44317344931). A shrink alone lost nothing on the Windows runner: 50 of 50 runs from the test
//! process (the daemon's `terminal_lines` probe, CI runs 44327828545 and 44338573473) and 4 of 4
//! in a detached daemon (run 44338573473).

mod common;
/// The one lock of the tests that open ptys, shared with the daemon's `runner.rs`.
#[path = "../../daemon/tests/support/mod.rs"]
mod pty_lock;

use std::time::{Duration, Instant};

use common::{Tok, repo};

/// A ceiling, not a delay.
const WAIT: Duration = Duration::from_secs(60);
/// Runs of each way.
const RUNS: usize = 4;
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

/// What a client does once the agent's first line is in the log.
#[derive(Debug, Clone, Copy)]
enum Opens {
    /// Sends the panel's size (152x14).
    Resizes,
    /// Types the answer to the cursor query, then sends the size: what the page does.
    AnswersAndResizes,
    /// Types the answer to the cursor query only.
    Answers,
}

/// [`RUNS`] runs of a paced 80-line agent per way of opening; the lines each run's log lacks.
fn runs(ways: &[Opens]) -> Vec<(Opens, Vec<Vec<u32>>)> {
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
    let mut n = 0;
    let mut out = Vec::new();
    for &way in ways {
        let mut lost = Vec::new();
        for _ in 0..RUNS {
            n += 1;
            let key = format!("DEMO-{n}");
            tok.run(&["ticket", "new", "demo", &format!("Run {n}")]);
            tok.run(&["start", &key]);
            wait_until("the first line", || tok.logs(&key).contains("scroll line"));
            let client = tok.client();
            if matches!(way, Opens::Answers | Opens::AnswersAndResizes) {
                client.runner_input(&key, b"\x1b[1;1R").expect("answer");
            }
            if matches!(way, Opens::Resizes | Opens::AnswersAndResizes) {
                client.runner_size(&key, &small).expect("resize");
            }
            wait_until("the last line", || tok.logs(&key).contains("printed all"));
            lost.push(missing(&tok.logs(&key)));
            tok.run(&["cancel", &key, "done"]);
        }
        out.push((way, lost));
    }
    out
}

#[test]
fn a_client_that_opens_the_terminal_while_the_agent_prints_costs_no_line_of_the_run_log() {
    let _turn = pty_lock::hold();
    let ways = [Opens::Resizes, Opens::AnswersAndResizes, Opens::Answers];
    let got = runs(&ways);
    let report = format!("{got:?}");
    eprintln!("TOKNILA-162 detached daemon, lines missing per run: {report}");
    let lost = got.iter().flat_map(|(_, runs)| runs).any(|r| !r.is_empty());
    // A Unix pty is a byte pipe and its program reads no input here: nothing is lost (the control).
    if cfg!(unix) {
        assert!(!lost, "{report}");
    }
}
