//! P1-03: threads on a card through the real `tok` binary and daemon. The simulated agent of a
//! running ticket opens a design thread and posts a mermaid version; its hand-in is refused while
//! the thread is open; the owner requests changes, the agent posts version 2, the owner approves
//! and the hand-in moves the ticket to Review. The `--json` output is pinned by snapshots in
//! `tests/snapshots/`.

mod common;

use common::{Tok, snapshot};

const CHAIN: &str = r#"
[chain]
name = "demo"
goal = "A list screen"

[[ticket]]
id = "LIST"
title = "List screen"
profile = "sim"
"#;

const V1: &str = "flowchart TD\n  A[List] --> B[Item]\n";
const V2: &str = "flowchart TD\n  A[List] --> B[Item]\n  A --> E[Empty state]\n";

#[test]
fn the_hand_in_waits_for_the_owner_to_approve_the_design() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tok = Tok {
        home: dir.path().join("home"),
    };
    let chain = dir.path().join("demo.toml");
    std::fs::write(&chain, CHAIN).expect("write chain");
    let v1 = dir.path().join("v1.mmd");
    let v2 = dir.path().join("v2.mmd");
    std::fs::write(&v1, V1).expect("write v1");
    std::fs::write(&v2, V2).expect("write v2");
    let (v1, v2) = (v1.to_str().expect("utf-8"), v2.to_str().expect("utf-8"));
    tok.run(&["project", "new", "DEMO", "--name", "Demo"]);
    tok.run(&[
        "chain",
        "import",
        chain.to_str().expect("utf-8 path"),
        "--project",
        "DEMO",
    ]);
    assert_eq!(tok.statuses(), ["DEMO-1 running"]);
    assert_eq!(tok.run(&["show", "DEMO-1"])["phase"], "implement");

    // 1. The agent opens a design thread and posts version 1: the ticket is in its design phase.
    let opened = tok.run(&["sim", "DEMO-1", "thread", "open", "design", "List screen"]);
    assert_eq!(opened["phase"], "design");
    let posted = tok.run(&[
        "sim",
        "DEMO-1",
        "thread",
        "post",
        "1",
        "--artifact",
        v1,
        "--format",
        "mermaid",
        "--message",
        "empty, loading, error and content states to follow",
    ]);
    assert_eq!(posted["ticket"]["status"], "running");

    // 2. The hand-in is refused with the open thread named, and nothing moves.
    let msg = tok.fails(&["sim", "DEMO-1", "finish", "done"]);
    assert!(
        msg.contains("cannot hand in DEMO-1: 1 open: #1 \"List screen\" (Design)"),
        "{msg}"
    );
    assert_eq!(tok.status("DEMO-1"), "running");
    let list = tok.run(&["thread", "ls", "DEMO-1"]);
    assert_eq!(list["phase"], "design");
    let th = &list["threads"][0];
    assert_eq!(
        (
            &th["thread"]["state"],
            &th["thread"]["opened_by"],
            &th["versions"][0]["number"],
            &th["versions"][0]["format"],
            &th["versions"][0]["body"],
            &th["messages"][0]["author"],
        ),
        (
            &"open".into(),
            &"agent".into(),
            &1.into(),
            &"mermaid".into(),
            &V1.into(),
            &"agent".into()
        )
    );
    snapshot("thread_list_open", list);

    // 3. The owner asks for changes; the thread stays open and the agent answers with version 2.
    let changes = tok.run(&["thread", "changes", "DEMO-1", "1", "show the empty state"]);
    assert_eq!(changes["thread"]["state"], "open");
    assert_eq!(changes["messages"][1]["kind"], "changes_requested");
    assert_eq!(changes["messages"][1]["author"], "human");
    snapshot("thread_changes_requested", changes);
    tok.run(&[
        "sim",
        "DEMO-1",
        "thread",
        "post",
        "1",
        "--artifact",
        v2,
        "--format",
        "mermaid",
    ]);
    assert!(
        tok.fails(&["sim", "DEMO-1", "finish", "done"])
            .contains("1 open"),
        "still open until a person approves"
    );

    // 4. The owner approves: version 2 is pinned and the hand-in goes through.
    let approved = tok.run(&["thread", "approve", "DEMO-1", "1"]);
    assert_eq!(
        (
            &approved["thread"]["state"],
            &approved["thread"]["approved_version"]
        ),
        (&"approved".into(), &2.into())
    );
    assert_eq!(approved["versions"].as_array().map(Vec::len), Some(2));
    snapshot("thread_approved", approved);
    assert_eq!(tok.run(&["show", "DEMO-1"])["phase"], "implement");
    let done = tok.run(&["sim", "DEMO-1", "finish", "list screen done"]);
    assert_eq!(done["ticket"]["status"], "review");
    assert_eq!(done["phase"], "implement");
    assert_eq!(tok.chain()["tickets"][0]["status"], "review");

    // 5. A new version from the agent reopens an approved thread; only a person closes it again.
    let reopened = tok.run(&[
        "sim",
        "DEMO-1",
        "thread",
        "post",
        "1",
        "--message",
        "one more idea",
    ]);
    assert_eq!(reopened["phase"], "implement", "a message does not reopen");
    tok.run(&[
        "sim",
        "DEMO-1",
        "thread",
        "post",
        "1",
        "--artifact",
        v1,
        "--format",
        "mermaid",
    ]);
    let list = tok.run(&["thread", "ls", "DEMO-1"]);
    assert_eq!(
        (
            &list["phase"],
            &list["threads"][0]["thread"]["approved_version"]
        ),
        (&"design".into(), &serde_json::Value::Null)
    );
    assert_eq!(
        list["threads"][0]["versions"].as_array().map(Vec::len),
        Some(3)
    );
}

#[test]
fn a_person_opens_a_thread_and_approves_it_in_words() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tok = Tok {
        home: dir.path().join("home"),
    };
    let chain = dir.path().join("demo.toml");
    std::fs::write(&chain, CHAIN).expect("write chain");
    tok.run(&["project", "new", "DEMO", "--name", "Demo"]);
    tok.run(&[
        "chain",
        "import",
        chain.to_str().expect("utf-8 path"),
        "--project",
        "DEMO",
    ]);
    let opened = tok.run(&["thread", "open", "DEMO-1", "flow", "Sign-in flow"]);
    assert_eq!(
        (
            &opened["thread"]["number"],
            &opened["thread"]["kind"],
            &opened["thread"]["opened_by"]
        ),
        (&1.into(), &"flow".into(), &"human".into())
    );
    snapshot("thread_opened", opened);
    let msg = tok.fails(&["sim", "DEMO-1", "finish", "done"]);
    assert!(msg.contains("#1 \"Sign-in flow\" (Flow)"), "{msg}");
    let missing = tok.fails(&["thread", "approve", "DEMO-1", "9"]);
    assert!(
        missing.contains("thread 9 of DEMO-1 not found"),
        "{missing}"
    );
    let nothing = tok.fails(&["thread", "post", "DEMO-1", "1"]);
    assert!(nothing.contains("--message"), "{nothing}");
    // A message and an artifact go in one call.
    let file = dir.path().join("flow.md");
    std::fs::write(&file, "# Sign-in\n").expect("write");
    let posted = tok.run(&[
        "thread",
        "post",
        "DEMO-1",
        "1",
        "--artifact",
        file.to_str().expect("utf-8"),
        "--format",
        "markdown",
        "--message",
        "first sketch",
    ]);
    assert_eq!(
        (
            posted["versions"].as_array().map(Vec::len),
            posted["messages"].as_array().map(Vec::len),
            &posted["messages"][0]["text"]
        ),
        (Some(1), Some(1), &"first sketch".into())
    );
    let approved = tok.run(&["thread", "approve", "DEMO-1", "1"]);
    assert_eq!(approved["thread"]["approved_version"], 1);
    assert_eq!(
        tok.run(&["sim", "DEMO-1", "finish", "ok"])["ticket"]["status"],
        "review"
    );
}
