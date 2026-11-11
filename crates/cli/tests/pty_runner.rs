//! The PTY runner through the real `tok` binary and daemon, with the workspace's
//! `fake-agent` as the profile's command (no shell, no OS tools): a runner starts in its ship and
//! streams over the WebSocket, takes input, stops on cancel, a crash blocks the ticket, and a
//! daemon killed hard marks the tickets it was running `runner_lost` on restart.

mod common;

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use common::{Tok, git, kill_hard, repo};
use toknila_client::Client;
use toknila_daemon::info::DaemonInfo;

/// A ceiling, not a delay: every wait returns the moment its event happens. It is generous
/// because a loaded machine (a parallel build, a Windows runner) can take a minute to start a
/// daemon and a pty.
const WAIT: Duration = Duration::from_secs(120);

/// What a failed wait needs to be diagnosed from a CI log: the tickets and the daemon's logs.
fn diagnostics(tok: &Tok) -> String {
    let mut out = String::new();
    for key in ["DEMO-1", "DEMO-2", "DEMO-3"] {
        let shown = Command::new(env!("CARGO_BIN_EXE_tok"))
            .args(["show", key, "--json"])
            .env("TOKNILA_HOME", &tok.home)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        let ticket: serde_json::Value = serde_json::from_str(&shown).unwrap_or_default();
        out.push_str(&format!(
            "{key}: {} {}\n",
            ticket["ticket"]["status"], ticket["ticket"]["status_reason"]
        ));
    }
    let entries = std::fs::read_dir(tok.home.join("logs"))
        .into_iter()
        .flatten();
    for entry in entries.filter_map(Result::ok) {
        let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
        let tail: Vec<&str> = text.lines().rev().take(40).collect();
        out.push_str(&format!(
            "-- {}\n{}\n",
            entry.path().display(),
            tail.join("\n")
        ));
    }
    out
}

fn wait_until(tok: &Tok, what: &str, mut ok: impl FnMut() -> bool) {
    let until = Instant::now() + WAIT;
    while !ok() {
        assert!(
            Instant::now() < until,
            "timed out waiting for {what}\n{}",
            diagnostics(tok)
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn reason_of(tok: &Tok, key: &str) -> String {
    tok.run(&["show", key])["ticket"]["status_reason"].to_string()
}

/// `tok logs --follow`, with its lines delivered on a channel.
fn follow(home: &Path, ticket: &str) -> (Child, mpsc::Receiver<String>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["logs", ticket, "--follow"])
        .env("TOKNILA_HOME", home)
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn tok logs");
    let out = child.stdout.take().expect("stdout");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            let _ = tx.send(line);
        }
    });
    (child, rx)
}

fn wait_line(rx: &mpsc::Receiver<String>, want: &str) {
    let until = Instant::now() + WAIT;
    while Instant::now() < until {
        if let Ok(line) = rx.recv_timeout(Duration::from_millis(200))
            && line.contains(want)
        {
            return;
        }
    }
    panic!("no line with {want:?} on the stream");
}

#[test]
fn a_runner_streams_takes_input_stops_on_cancel_crashes_and_is_lost_with_the_daemon() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    let repo = repo(dir.path());
    tok.profile(
        "chatty",
        &[
            "print",
            "hello from the agent",
            "pwd",
            "echo",
            "sleep",
            "30000",
        ],
        "",
    );
    tok.profile("crashy", &["print", "boom", "exit", "3"], "");
    tok.profile("lost", &["print", "i will be lost", "sleep", "30000"], "");

    tok.run(&[
        "project",
        "new",
        "DEMO",
        "--repo",
        &repo.to_string_lossy(),
        "--profile",
        "chatty",
    ]);
    tok.run(&["chain", "new", "demo"]);
    // Autostart: each ticket is Running, with its ship made and its runner started, when the
    // command returns.
    tok.run(&["ticket", "new", "demo", "Alpha"]);
    tok.run(&["ticket", "new", "demo", "Crash", "--profile", "crashy"]);
    tok.run(&["ticket", "new", "demo", "Lost", "--profile", "lost"]);

    // Output reaches the scrollback, and the runner works in the ship, not in the repository.
    wait_until(&tok, "the agent's output", || {
        tok.logs("DEMO-1").contains("hello from the agent")
    });
    let out = tok.logs("DEMO-1");
    assert!(out.contains("demo-1-alpha"), "pwd is the ship: {out:?}");

    // Live over the WebSocket, with a late client seeing history first, then typing into it.
    let (mut follower, lines) = follow(&tok.home, "DEMO-1");
    wait_line(&lines, "hello from the agent");
    let info = DaemonInfo::read(&tok.home.join("daemon.json"))
        .expect("daemon.json")
        .expect("daemon runs");
    let base = info.base_url();
    let client = Client::new(base.clone(), info.token.clone());
    client.runner_input("DEMO-1", b"ping\r").expect("input");
    wait_line(&lines, "got:ping");

    // StopRunner: cancelling stops the process, and the follower sees the run end.
    tok.run(&["cancel", "DEMO-1", "not needed"]);
    wait_until(&tok, "the follower to end", || {
        follower.try_wait().expect("wait").is_some()
    });
    assert!(follower.wait().expect("status").success());
    assert_eq!(tok.status("DEMO-1"), "cancelled");
    assert!(tok.logs("DEMO-1").contains("got:ping"), "history stays");

    // A crash comes back through the machine: exit 3 blocks the ticket, saying why.
    wait_until(&tok, "DEMO-2 to block", || {
        tok.status("DEMO-2") == "blocked"
    });
    assert!(
        reason_of(&tok, "DEMO-2").contains("exit 3"),
        "{}",
        reason_of(&tok, "DEMO-2")
    );
    assert!(tok.logs("DEMO-2").contains("boom"));

    // Killing the daemon loses the runner of DEMO-3; the restart blocks it as runner_lost.
    assert_eq!(tok.status("DEMO-3"), "running");
    let pid = info.pid;
    kill_hard(pid);
    wait_until(&tok, "the daemon to die", || !Client::healthy(&base));
    // Any command starts the daemon again; recovery runs before it answers.
    assert_eq!(tok.status("DEMO-3"), "blocked");
    assert!(
        reason_of(&tok, "DEMO-3").contains("runner_lost"),
        "{}",
        reason_of(&tok, "DEMO-3")
    );
    assert_eq!(tok.status("DEMO-1"), "cancelled", "nothing else moved");
}

#[test]
fn a_persons_instruction_is_typed_to_the_agent_once_and_the_terminal_follows_the_panel() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    let repo = repo(dir.path());
    // No hooks are wired, so the way in is the terminal.
    tok.profile(
        "listener",
        &["print", "ready", "echo", "size", "sleep", "30000"],
        "",
    );
    tok.run(&[
        "project",
        "new",
        "DEMO",
        "--repo",
        &repo.to_string_lossy(),
        "--profile",
        "listener",
    ]);
    tok.run(&["chain", "new", "demo"]);
    tok.run(&["ticket", "new", "demo", "Alpha"]);
    wait_until(&tok, "the agent to be ready", || {
        tok.logs("DEMO-1").contains("ready")
    });

    tok.client()
        .runner_size(
            "DEMO-1",
            &toknila_api_types::TerminalSize { cols: 90, rows: 30 },
        )
        .expect("resize");

    tok.run(&["comment", "DEMO-1", "use rustls"]);
    wait_until(&tok, "the instruction to reach the agent", || {
        tok.logs("DEMO-1")
            .contains("got:# [instruction] use rustls")
    });
    wait_until(&tok, "the terminal to have the panel's size", || {
        tok.logs("DEMO-1").contains("size=90x30")
    });
    wait_until(&tok, "the delivered mark", || {
        let shown = tok.run(&["show", "DEMO-1"]);
        let comments = shown["comments"].as_array().cloned().unwrap_or_default();
        comments.len() == 1 && !comments[0]["delivered_at"].is_null()
    });
    tok.run(&["cancel", "DEMO-1", "done"]);
}

#[test]
fn a_return_goes_to_the_same_live_agent_session_and_is_delivered_once() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    let repo = repo(dir.path());
    // The agent stays alive after its hand-in and reads one line, as a session would.
    tok.profile(
        "listener",
        &["print", "ready", "echo", "sleep", "30000"],
        "",
    );
    tok.run(&[
        "project",
        "new",
        "DEMO",
        "--repo",
        &repo.to_string_lossy(),
        "--profile",
        "listener",
    ]);
    tok.run(&["chain", "new", "demo"]);
    tok.run(&["ticket", "new", "demo", "Alpha"]);
    wait_until(&tok, "the agent to be ready", || {
        tok.logs("DEMO-1").contains("ready")
    });
    let first_run = tok.run(&["show", "DEMO-1"])["ship"]["path"].clone();

    let ship = PathBuf::from(first_run.as_str().expect("ship path"));
    std::fs::write(ship.join("alpha.txt"), "alpha\n").expect("write");
    git(&ship, &["add", "alpha.txt"]);
    git(&ship, &["commit", "--quiet", "-m", "alpha"]);
    tok.run(&["sim", "DEMO-1", "finish", "first try"]);
    assert_eq!(tok.status("DEMO-1"), "review");
    tok.wait_for_person_turn("DEMO-1");
    tok.run(&["return", "DEMO-1", "fix the look"]);
    wait_until(&tok, "the comment to reach the same session", || {
        tok.logs("DEMO-1")
            .contains("got:# [instruction] fix the look")
    });
    wait_until(&tok, "the delivered mark", || {
        let shown = tok.run(&["show", "DEMO-1"]);
        let comments = shown["comments"].as_array().cloned().unwrap_or_default();
        comments
            .iter()
            .filter(|c| c["kind"] == "instruction")
            .all(|c| !c["delivered_at"].is_null())
            && comments.iter().any(|c| c["kind"] == "instruction")
    });
    assert_eq!(tok.status("DEMO-1"), "running");
    assert_eq!(
        tok.run(&["show", "DEMO-1"])["ship"]["path"],
        first_run,
        "the same ship"
    );
    tok.run(&["cancel", "DEMO-1", "done"]);
}
