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
    let dir = common::tempdir();
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
                choice: None,
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
    let dir = common::tempdir();
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
                choice: None,
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
fn a_question_asked_after_the_silent_nudge_reaches_the_inbox_and_the_answer_reaches_the_agent() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    // A first hook (the agent has shown life), then quiet for a second and no nudges allowed: the
    // first look of the ticker (every 250 ms in a test daemon) gives up and the card waits as
    // silent; the 4 s sleep leaves it room even under load (a sleep only gets longer). The agent then
    // asks, still alive, and reads the answer. (A run with no sign of life is never given up on.)
    tok.profile(
        "quiet",
        &[
            "post",
            "/tickets/{ticket}/activity",
            r#"{"signal":"hook:stop"}"#,
            "sleep",
            "4000",
            "post",
            "/tickets/{ticket}/events",
            r#"{"event":"ask_human","text":"which db?"}"#,
            "echo",
            "sleep",
            "30000",
        ],
        "[nudge]\nidle_secs = 1\nmax_nudges = 0\n",
    );
    setup(&tok, dir.path(), "quiet");
    let client = tok.client();

    wait_until("the silent wait", || tok.status("DEMO-1") == "waiting");
    assert!(questions(&client).is_empty(), "nothing is asked yet");

    wait_until("the question", || !questions(&client).is_empty());
    let q = questions(&client);
    assert_eq!(q.len(), 1);
    assert_eq!(q[0].text, "which db?");
    assert_eq!(tok.status("DEMO-1"), "waiting");

    // A question of ask_human is answered with a comment, typed into the agent's terminal.
    tok.run(&["comment", "DEMO-1", "sqlite"]);
    wait_until("the agent to get the answer", || {
        tok.logs("DEMO-1").contains("got:")
    });
    assert!(
        tok.logs("DEMO-1").contains("sqlite"),
        "{}",
        tok.logs("DEMO-1")
    );
    assert_eq!(tok.status("DEMO-1"), "running");
    assert!(
        questions(&client).is_empty(),
        "the answered question is gone"
    );
    tok.run(&["cancel", "DEMO-1", "done"]);
}

#[test]
fn tok_show_numbers_the_choices_of_a_question_and_tok_answer_picks_one_by_number() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    tok.profile(
        "chooser",
        &[
            "post",
            "/tickets/{ticket}/events",
            r#"{"event":"ask_human","text":"Check the phone?","choices":["Hand in now","Wait for the phone check"]}"#,
            "echo",
            "sleep",
            "30000",
        ],
        "",
    );
    setup(&tok, dir.path(), "chooser");
    let client = tok.client();
    wait_until("the question", || !questions(&client).is_empty());
    let shown = tok.text(&["show", "DEMO-1"]);
    assert!(shown.contains("  1. Hand in now (recommended)"), "{shown}");
    assert!(shown.contains("  2. Wait for the phone check"), "{shown}");
    tok.run(&["answer", "DEMO-1", "2"]);
    wait_until("the agent to get the choice", || {
        tok.logs("DEMO-1").contains("Wait for the phone check")
    });
    wait_until("the card to run again", || {
        tok.status("DEMO-1") == "running"
    });
    tok.run(&["cancel", "DEMO-1", "done"]);
}

#[test]
fn a_long_permission_request_shows_its_full_command_in_the_inbox_and_in_tok_show() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    let command = format!("echo start\\n{}\\necho EOF", "echo 1\\n".repeat(100));
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
    assert!(detail.starts_with("Bash: echo start\n"), "{detail}");
    assert!(
        detail.ends_with("\necho EOF"),
        "the end is not cut: {detail}"
    );
    assert!(detail.chars().count() > 600);

    let shown = tok.text(&["show", "DEMO-1"]);
    assert!(shown.contains("the full request:"), "{shown}");
    assert!(shown.contains("  echo EOF"), "{shown}");
    tok.run(&["cancel", "DEMO-1", "done"]);
}

#[test]
fn the_worker_process_ends_at_the_hand_in_and_a_return_starts_a_new_run_with_the_earlier_instructions()
 {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    // It prints its instructions, then waits for a line and asks to run something.
    tok.profile(
        "verifier",
        &[
            "print",
            "ready",
            "pid",
            "flagfile",
            "--instr",
            "echo",
            "post",
            "/tickets/{ticket}/permission",
            r#"{"tool":"Bash: cargo xtask verify"}"#,
            "sleep",
            "30000",
        ],
        "instructions_flag = \"--instr\"\n",
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
    tok.run(&["comment", "DEMO-1", "use the Edit tool"]);
    // The command answers before the instruction is typed; only a delivered one is carried
    // over to a new run.
    wait_until("the instruction to be delivered", || {
        tok.run(&["show", "DEMO-1"])["comments"]
            .as_array()
            .is_some_and(|c| {
                c.iter()
                    .any(|c| c["kind"] == "instruction" && !c["delivered_at"].is_null())
            })
    });
    tok.run(&["sim", "DEMO-1", "finish", "done"]);
    assert_eq!(tok.status("DEMO-1"), "review");

    // The hand-in ends the run: its process is gone within seconds, nobody was asked.
    wait_until("the worker process to end", || {
        client.runner_input("DEMO-1", b"go\r").is_err()
    });
    assert_eq!(tok.status("DEMO-1"), "review", "an ended run is no crash");
    assert!(questions(&client).is_empty());

    // Returned, a new run starts with the earlier instructions, and its request waits for a
    // person.
    tok.wait_for_person_turn("DEMO-1");
    tok.run(&["return", "DEMO-1", "one more thing"]);
    wait_until("the new run to print its instructions", || {
        let log = tok.logs("DEMO-1");
        log.contains("file:## Instructions you were given earlier")
            && log.contains("use the Edit tool")
    });
    client.runner_input("DEMO-1", b"go\r").expect("input");
    wait_until("the question", || !questions(&client).is_empty());
    assert_eq!(tok.status("DEMO-1"), "waiting");
    tok.run(&["cancel", "DEMO-1", "done"]);
}

#[test]
fn mcp_events_after_a_question_leave_the_card_waiting_on_it() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    let events = "/tickets/{ticket}/events";
    tok.profile(
        "chatty",
        &[
            "post",
            events,
            r#"{"event":"ask_human","text":"which db?"}"#,
            "post",
            events,
            r#"{"event":"started"}"#,
            "post",
            events,
            r#"{"event":"note","body":{"gist":"still here"}}"#,
            "sleep",
            "30000",
        ],
        "",
    );
    setup(&tok, dir.path(), "chatty");
    let client = tok.client();

    // All three posts were answered (the question, the start report, the note) ...
    wait_until("the agent's posts", || {
        tok.logs("DEMO-1").matches("post=200").count() == 3
    });
    // ... and the card is still on its question: only the answer closes it.
    assert_eq!(tok.status("DEMO-1"), "waiting");
    let q = questions(&client);
    assert_eq!(q.len(), 1);
    assert_eq!(q[0].text, "which db?");
    tok.run(&["cancel", "DEMO-1", "done"]);
}
