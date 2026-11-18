//! TOKNILA-162: a panel's size that comes while the agent prints, in a real daemon started as
//! `tok` starts it (detached, with no console of its own on Windows), as the page sends it when it
//! opens. The run log must keep every line the agent printed.
//!
//! The terminal-copy spec's run log lacked lines 1-12 and 1-8 in two of six Windows runs (CI run
//! 44317344931) after the page sent 152x14 while the agent printed, while a ConPTY opened by the
//! test process itself lost nothing when shrunk mid-output (25 of 25 runs, the daemon's
//! `terminal_lines` probe). This test runs the same agent with the shrink sent at once
//! (`TOKNILA_RESIZE_SETTLE_MS=0`, as before) and as the runner holds it now (until the output has
//! been quiet), and reports both.

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

/// [`RUNS`] runs of a paced 80-line agent, each shrunk to 152x14 once its first line is in the
/// log; the lines each run's log lacks.
fn runs(env: &[(&str, &str)]) -> Vec<Vec<u32>> {
    let dir = common::tempdir();
    let tok = Tok::in_dir_with(dir.path(), env);
    let repo = repo(dir.path());
    let mut steps = vec!["sleep".to_owned(), "0".to_owned()];
    for i in 1..=LINES {
        steps.extend(["print".to_owned(), format!("scroll line {i}")]);
        steps.extend(["sleep".to_owned(), "2".to_owned()]);
    }
    steps.extend([
        "print".to_owned(),
        "printed all".to_owned(),
        "hold".to_owned(),
    ]);
    let steps: Vec<&str> = steps.iter().map(String::as_str).collect();
    tok.profile("printer", &steps, "");
    let repo = repo.to_string_lossy();
    tok.run(&[
        "project",
        "new",
        "DEMO",
        "--repo",
        &repo,
        "--profile",
        "printer",
    ]);
    tok.run(&["chain", "new", "demo", "--no-autostart"]);
    let mut lost = Vec::new();
    for n in 1..=RUNS {
        let key = format!("DEMO-{n}");
        tok.run(&["ticket", "new", "demo", &format!("Run {n}")]);
        tok.run(&["start", &key]);
        wait_until("the first line", || tok.logs(&key).contains("scroll line"));
        let small = toknila_api_types::TerminalSize {
            cols: 152,
            rows: 14,
        };
        tok.client().runner_size(&key, &small).expect("resize");
        wait_until("the last line", || tok.logs(&key).contains("printed all"));
        lost.push(missing(&tok.logs(&key)));
        tok.run(&["cancel", &key, "done"]);
    }
    lost
}

#[test]
fn a_size_sent_while_the_agent_prints_costs_no_line_of_the_run_log() {
    let _turn = pty_lock::hold();
    let at_once = runs(&[("TOKNILA_RESIZE_SETTLE_MS", "0")]);
    let held = runs(&[]);
    let report = format!("sent at once: {at_once:?}; held until quiet: {held:?}");
    eprintln!("TOKNILA-162 detached daemon, lines missing per run: {report}");
    assert!(held.iter().all(Vec::is_empty), "{report}");
    // A Unix pty is a byte pipe: the control that a shrink sent at once loses nothing there.
    if cfg!(unix) {
        assert!(at_once.iter().all(Vec::is_empty), "{report}");
    }
}
