//! What an agent asks and a person answers, through the real `tok` binary, a daemon and the
//! workspace's `fake-agent` on a pseudo-terminal (never a real agent): its permission hook
//! waits for the window's decision; an agent that shows nothing is listed with its screen and
//! answered by keys; typing goes to the role it is addressed to.

mod common;

use std::path::Path;
use std::time::{Duration, Instant};

use common::{Tok, repo};
use toknila_api_types::NewAnswer;
use toknila_client::Client;
use toknila_core::inbox::{InboxItem, InboxKind};
use toknila_core::question::AnswerKind;

const WAIT: Duration = Duration::from_secs(30);

fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let until = Instant::now() + WAIT;
    while !ok() {
        assert!(Instant::now() < until, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
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
    let tok = Tok::in_dir(dir.path());
    tok.profile(
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
    let client = tok.client();

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
                always: false,
            },
        )
        .expect("answer");
    assert_eq!(
        tok.status("DEMO-1"),
        "running",
        "at once, not when a tool ends"
    );
    wait_until("the hook's reply", || {
        tok.logs("DEMO-1")
            .contains(r#"post=200 {"decision":"allow"}"#)
    });
    assert!(questions(&client).is_empty());
}

#[test]
fn an_agent_that_shows_nothing_is_listed_with_its_screen_and_answered_by_keys() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tok = Tok::in_dir(dir.path());
    tok.profile(
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
    let client = tok.client();

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
                always: false,
            },
        )
        .expect("answer");
    wait_until("the agent to get the text", || {
        tok.logs("DEMO-1").contains("got:yes")
    });
    // The screen changed with the answer and the agent still shows no sign of life: the new
    // screen is listed at once (several keys can follow one another).
    wait_until("the changed screen", || {
        questions(&client)
            .first()
            .is_some_and(|q| q.lines.iter().any(|l| l.contains("got:yes")))
    });
}

#[test]
fn a_long_permission_request_shows_its_full_command_in_the_inbox_and_in_tok_show() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tok = Tok::in_dir(dir.path());
    let command = format!("python3 - <<EOF\\n{}\\nEOF", "print(1)\\n".repeat(100));
    let body = format!(r#"{{"tool":"Bash: {command}"}}"#);
    tok.profile(
        "hooked",
        &[
            "post",
            "/tickets/{ticket}/permission",
            &body,
            "sleep",
            "30000",
        ],
        "",
    );
    setup(&tok, dir.path(), "hooked");
    let client = tok.client();

    wait_until("the permission question", || !questions(&client).is_empty());
    let q = questions(&client);
    assert!(q[0].text.chars().count() < 340, "{}", q[0].text);
    let detail = q[0]
        .detail
        .clone()
        .expect("the full command travels with it");
    assert!(detail.starts_with("Bash: python3 - <<EOF\n"), "{detail}");
    assert!(detail.ends_with("\nEOF"), "the end is not cut: {detail}");
    assert!(detail.chars().count() > 600);

    let shown = tok.text(&["show", "DEMO-1"]);
    assert!(shown.contains("the full request:"), "{shown}");
    assert!(shown.contains("  EOF"), "{shown}");
    tok.run(&["cancel", "DEMO-1", "done"]);
}

#[test]
fn an_agent_that_asks_after_its_hand_in_is_refused_with_a_reason_and_asks_again_once_returned() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tok = Tok::in_dir(dir.path());
    // It waits for a line, then asks to run something, twice: the first ask comes after the
    // hand-in, the second after the card was returned.
    tok.profile(
        "verifier",
        &[
            "print",
            "ready",
            "echo",
            "post",
            "/tickets/{ticket}/permission",
            r#"{"tool":"Bash: cargo xtask verify"}"#,
            "echo",
            "post",
            "/tickets/{ticket}/permission",
            r#"{"tool":"Bash: cargo xtask verify"}"#,
            "sleep",
            "30000",
        ],
        "",
    );
    setup(&tok, dir.path(), "verifier");
    let client = tok.client();
    wait_until("the agent to be ready", || {
        tok.logs("DEMO-1").contains("ready")
    });
    let ship = std::path::PathBuf::from(
        tok.run(&["show", "DEMO-1"])["ship"]["path"]
            .as_str()
            .expect("ship"),
    );
    std::fs::write(ship.join("a.txt"), "a\n").expect("write");
    common::git(&ship, &["add", "a.txt"]);
    common::git(&ship, &["commit", "--quiet", "-m", "a"]);
    tok.run(&["sim", "DEMO-1", "finish", "done"]);
    assert_eq!(tok.status("DEMO-1"), "review");

    client.runner_input("DEMO-1", b"go\r").expect("input");
    wait_until("the refusal", || {
        tok.logs("DEMO-1").contains(r#""decision":"deny""#)
    });
    let log = tok.logs("DEMO-1");
    assert!(
        log.contains("handed in") && log.contains("unless the card is returned"),
        "{log}"
    );
    assert_eq!(tok.status("DEMO-1"), "review", "nobody was asked");
    assert!(questions(&client).is_empty());

    // Returned, the card is the worker's again and its next request waits for a person.
    tok.run(&["return", "DEMO-1", "one more thing"]);
    client.runner_input("DEMO-1", b"go\r").expect("input");
    wait_until("the question", || !questions(&client).is_empty());
    assert_eq!(tok.status("DEMO-1"), "waiting");
    tok.run(&["cancel", "DEMO-1", "done"]);
}
