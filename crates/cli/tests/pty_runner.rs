//! P1-07: the PTY runner through the real `tok` binary and daemon, with the workspace's
//! `fake-agent` as the profile's command (no shell, no OS tools): a runner starts in its ship and
//! streams over the WebSocket, takes input, stops on cancel, a crash blocks the ticket, and a
//! daemon killed hard marks the tickets it was running `runner_lost` on restart.

mod common;

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use common::Tok;
use toknila_client::Client;
use toknila_daemon::info::DaemonInfo;

const WAIT: Duration = Duration::from_secs(30);

/// `fake-agent` is built by the daemon's own tests; build it here if this test runs alone.
fn fake_agent() -> PathBuf {
    let exe = format!("fake-agent{}", std::env::consts::EXE_SUFFIX);
    let path = Path::new(env!("CARGO_BIN_EXE_tok"))
        .parent()
        .expect("target dir")
        .join(exe);
    if !path.exists() {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
        let status = Command::new(cargo)
            .args([
                "build",
                "--locked",
                "-p",
                "toknila-daemon",
                "--bin",
                "fake-agent",
            ])
            .status()
            .expect("run cargo");
        assert!(status.success(), "cannot build fake-agent");
    }
    path
}

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

/// `<home>/profiles/<name>.toml` running the fake agent with `steps`.
fn profile(tok: &Tok, name: &str, steps: &[&str]) {
    let args: String = steps.iter().map(|s| format!(", '{s}'")).collect();
    let folder = tok.home.join("profiles");
    std::fs::create_dir_all(&folder).expect("mkdir");
    let text = format!("command = ['{}'{args}]\n", fake_agent().display());
    std::fs::write(folder.join(format!("{name}.toml")), text).expect("profile");
}

fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let until = Instant::now() + WAIT;
    while !ok() {
        assert!(Instant::now() < until, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn logs(home: &Path, ticket: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["logs", ticket])
        .env("TOKNILA_HOME", home)
        .output()
        .expect("run tok logs");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn reason_of(tok: &Tok, key: &str) -> String {
    tok.run(&["show", key])["ticket"]["status_reason"].to_string()
}

fn kill_hard(pid: u32) {
    let pid = pid.to_string();
    #[cfg(unix)]
    let out = Command::new("kill").args(["-9", &pid]).output();
    #[cfg(windows)]
    let out = Command::new("taskkill").args(["/F", "/PID", &pid]).output();
    out.expect("run kill");
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
    let dir = tempfile::tempdir().expect("tempdir");
    let tok = Tok {
        home: dir.path().join("home"),
    };
    let repo = repo(dir.path());
    profile(
        &tok,
        "chatty",
        &[
            "print",
            "hello from the agent",
            "pwd",
            "echo",
            "sleep",
            "30000",
        ],
    );
    profile(&tok, "crashy", &["print", "boom", "exit", "3"]);
    profile(&tok, "lost", &["print", "i will be lost", "sleep", "30000"]);

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
    wait_until("the agent's output", || {
        logs(&tok.home, "DEMO-1").contains("hello from the agent")
    });
    let out = logs(&tok.home, "DEMO-1");
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
    wait_until("the follower to end", || {
        follower.try_wait().expect("wait").is_some()
    });
    assert!(follower.wait().expect("status").success());
    assert_eq!(tok.status("DEMO-1"), "cancelled");
    assert!(
        logs(&tok.home, "DEMO-1").contains("got:ping"),
        "history stays"
    );

    // A crash comes back through the machine: exit 3 blocks the ticket, saying why.
    wait_until("DEMO-2 to block", || tok.status("DEMO-2") == "blocked");
    assert!(
        reason_of(&tok, "DEMO-2").contains("exit 3"),
        "{}",
        reason_of(&tok, "DEMO-2")
    );
    assert!(logs(&tok.home, "DEMO-2").contains("boom"));

    // Killing the daemon loses the runner of DEMO-3; the restart blocks it as runner_lost.
    assert_eq!(tok.status("DEMO-3"), "running");
    let pid = info.pid;
    kill_hard(pid);
    wait_until("the daemon to die", || !Client::healthy(&base));
    // Any command starts the daemon again; recovery runs before it answers.
    assert_eq!(tok.status("DEMO-3"), "blocked");
    assert!(
        reason_of(&tok, "DEMO-3").contains("runner_lost"),
        "{}",
        reason_of(&tok, "DEMO-3")
    );
    assert_eq!(tok.status("DEMO-1"), "cancelled", "nothing else moved");
}
