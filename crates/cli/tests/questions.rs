//! What an agent asks and a person answers, through the real `tok` binary, a daemon and the
//! workspace's `fake-agent` on a pseudo-terminal (never a real agent): its permission hook
//! waits for the window's decision; an agent that shows nothing is listed with its screen and
//! answered by keys; typing goes to the role it is addressed to.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use common::Tok;
use toknila_api_types::NewAnswer;
use toknila_client::Client;
use toknila_core::inbox::{InboxItem, InboxKind};
use toknila_core::question::AnswerKind;
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

/// `<home>/profiles/<name>.toml` running the fake agent with `steps`; `extra` is more TOML.
fn profile(tok: &Tok, name: &str, steps: &[&str], extra: &str) {
    let args: String = steps.iter().map(|s| format!(", '{s}'")).collect();
    let folder = tok.home.join("profiles");
    std::fs::create_dir_all(&folder).expect("mkdir");
    let text = format!("command = ['{}'{args}]\n{extra}", fake_agent().display());
    std::fs::write(folder.join(format!("{name}.toml")), text).expect("profile");
}

fn logs(home: &Path, ticket: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["logs", ticket])
        .env("TOKNILA_HOME", home)
        .output()
        .expect("run tok logs");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let until = Instant::now() + WAIT;
    while !ok() {
        assert!(Instant::now() < until, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn client(tok: &Tok) -> Client {
    let info = DaemonInfo::read(&tok.home.join("daemon.json"))
        .expect("daemon.json")
        .expect("daemon runs");
    Client::new(info.base_url(), info.token)
}

fn questions(client: &Client) -> Vec<InboxItem> {
    client
        .inbox("DEMO")
        .expect("inbox")
        .into_iter()
        .filter(|i| i.kind == InboxKind::Question)
        .collect()
}

fn setup(tok: &Tok, dir: &Path, profile_name: &str) {
    let repo = repo(dir);
    tok.run(&[
        "project",
        "new",
        "DEMO",
        "--repo",
        &repo.to_string_lossy(),
        "--profile",
        profile_name,
    ]);
    tok.run(&["chain", "new", "demo"]);
    tok.run(&["ticket", "new", "demo", "Alpha"]);
}

#[test]
fn the_permission_hook_waits_for_allow_from_the_window_and_the_card_resumes_at_once() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tok = Tok {
        home: dir.path().join("home"),
    };
    profile(
        &tok,
        "hooked",
        &[
            "post",
            "/tickets/{ticket}/activity",
            r#"{"signal":"hook:pre_tool_use","tool":"Bash: cargo test"}"#,
            "post",
            "/tickets/{ticket}/permission",
            r#"{}"#,
            "sleep",
            "30000",
        ],
        "",
    );
    setup(&tok, dir.path(), "hooked");
    let client = client(&tok);

    wait_until("the permission question", || !questions(&client).is_empty());
    let q = questions(&client);
    assert_eq!(q.len(), 1);
    assert_eq!(
        q[0].text,
        "Claude Code asks for permission: Bash: cargo test"
    );
    assert_eq!(q[0].answers, [AnswerKind::Allow, AnswerKind::Deny]);
    assert_eq!(tok.status("DEMO-1"), "waiting");

    client
        .answer(
            "DEMO-1",
            &NewAnswer {
                role: None,
                answer: AnswerKind::Allow,
                text: None,
            },
        )
        .expect("answer");
    assert_eq!(
        tok.status("DEMO-1"),
        "running",
        "at once, not when a tool ends"
    );
    wait_until("the hook's reply", || {
        logs(&tok.home, "DEMO-1").contains(r#"post=200 {"decision":"allow"}"#)
    });
    assert!(questions(&client).is_empty());
}

#[test]
fn an_agent_that_shows_nothing_is_listed_with_its_screen_and_answered_by_keys() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tok = Tok {
        home: dir.path().join("home"),
    };
    profile(
        &tok,
        "silent",
        &[
            "print",
            "Do you trust this folder?",
            "print",
            "1. Yes  2. No",
            "echo",
            "sleep",
            "30000",
        ],
        "startup_secs = 1\n[hooks]\nenabled = true\n",
    );
    setup(&tok, dir.path(), "silent");
    let client = client(&tok);

    wait_until("the screen question", || !questions(&client).is_empty());
    let q = questions(&client);
    assert_eq!(q[0].text, "The agent waits on its screen");
    assert_eq!(
        q[0].lines,
        ["Do you trust this folder?", "1. Yes  2. No"],
        "the screen, without escapes"
    );
    assert!(q[0].answers.contains(&AnswerKind::Enter));

    client
        .answer(
            "DEMO-1",
            &NewAnswer {
                role: None,
                answer: AnswerKind::Text,
                text: Some("yes".to_owned()),
            },
        )
        .expect("answer");
    wait_until("the agent to get the text", || {
        logs(&tok.home, "DEMO-1").contains("got:yes")
    });
    assert!(
        questions(&client).is_empty(),
        "an answer starts the wait over"
    );
}
