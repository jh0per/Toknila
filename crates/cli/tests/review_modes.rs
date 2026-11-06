//! P1-14: review modes through the real `tok` binary and daemon, with simulated reviewers: a
//! person alone (human), the reviewer agent alone (agent), and the agent first and then a person
//! who sees its remarks (both). The mode resolves ticket, then chain, then project.

mod common;

use common::Tok;
use serde_json::Value;

const CHAIN: &str = r#"
[chain]
name = "demo"
goal = "Review modes"
autostart = false

[[ticket]]
id = "H"
title = "Human reviewed"
profile = "sim"

[[ticket]]
id = "A"
title = "Agent reviewed"
profile = "sim"

[[ticket]]
id = "B"
title = "Both review"
profile = "sim"
"#;

fn setup() -> (tempfile::TempDir, Tok) {
    let dir = tempfile::tempdir().expect("tempdir");
    let tok = Tok {
        home: dir.path().join("home"),
    };
    let file = dir.path().join("demo.toml");
    std::fs::write(&file, CHAIN).expect("write chain");
    tok.run(&["project", "new", "DEMO", "--name", "Demo"]);
    tok.run(&[
        "chain",
        "import",
        file.to_str().expect("utf-8 path"),
        "--project",
        "DEMO",
    ]);
    (dir, tok)
}

/// Starts the ticket and has its simulated agent hand it in.
fn hand_in(tok: &Tok, key: &str) -> Value {
    tok.run(&["start", key]);
    tok.run(&["sim", key, "finish", "done"])
}

fn rounds(detail: &Value) -> Vec<String> {
    detail["review"]["rounds"]
        .as_array()
        .expect("rounds")
        .iter()
        .map(|r| {
            format!(
                "{}:{}:{}:{}",
                r["number"],
                r["reviewer"].as_str().unwrap_or("?"),
                r["verdict"].as_str().unwrap_or("?"),
                r["comment"].as_str().unwrap_or("?")
            )
        })
        .collect()
}

#[test]
fn the_three_modes_with_simulated_reviewers() {
    let (_dir, tok) = setup();
    // Ticket over chain over project: H is its own, A follows the chain, B the project.
    tok.run(&["review-mode", "both", "--project", "DEMO"]);
    tok.run(&["review-mode", "agent", "--chain", "demo"]);
    tok.run(&["review-mode", "human", "--ticket", "DEMO-1"]);
    let modes: Vec<String> = ["DEMO-1", "DEMO-2", "DEMO-3"]
        .iter()
        .map(|k| tok.run(&["show", k])["review"]["mode"].to_string())
        .collect();
    assert_eq!(modes, ["\"human\"", "\"agent\"", "\"agent\""]);
    tok.run(&["review-mode", "both", "--ticket", "DEMO-3"]);

    // Human: the card waits for a person, who accepts with remarks.
    let h = hand_in(&tok, "DEMO-1");
    assert_eq!(h["review"]["awaiting"]["by"], "person");
    assert!(
        tok.fails(&["sim", "DEMO-1", "review", "approve"])
            .contains("review_verdict")
    );
    // The accept answers with the card merging; the merge's outcome follows it.
    let accepted = tok.run(&["accept", "DEMO-1", "well done"]);
    assert_eq!(accepted["review"]["awaiting"]["by"], "merging");
    let done = tok.run(&["show", "DEMO-1"]);
    assert_eq!(done["ticket"]["status"], "merged");
    assert_eq!(rounds(&done), ["1:human:approve:well done"]);

    // Agent: the reviewer's remarks send the card back, its approval merges it.
    let a = hand_in(&tok, "DEMO-2");
    assert_eq!(a["review"]["awaiting"]["decides"], true);
    let back = tok.run(&[
        "sim",
        "DEMO-2",
        "review",
        "request-changes",
        "handle the empty list",
    ]);
    assert_eq!(back["ticket"]["status"], "running");
    tok.run(&["sim", "DEMO-2", "finish", "fixed"]);
    tok.run(&["sim", "DEMO-2", "review", "approve", "now fine"]);
    let merged = tok.run(&["show", "DEMO-2"]);
    assert_eq!(merged["ticket"]["status"], "merged");
    assert_eq!(
        rounds(&merged),
        [
            "1:agent:request_changes:handle the empty list",
            "2:agent:approve:now fine"
        ]
    );

    // Both: the agent speaks first, the card then waits for a person who sees its remarks.
    let b = hand_in(&tok, "DEMO-3");
    assert_eq!(b["review"]["awaiting"]["decides"], false);
    let waits = tok.run(&["sim", "DEMO-3", "review", "request-changes", "rename it"]);
    assert_eq!(waits["ticket"]["status"], "review");
    assert_eq!(waits["review"]["awaiting"]["by"], "person");
    assert_eq!(rounds(&waits), ["1:agent:request_changes:rename it"]);
    let sent_back = tok.run(&["return", "DEMO-3", "agreed, and fix the look"]);
    assert_eq!(sent_back["ticket"]["status"], "running");
    assert_eq!(
        rounds(&sent_back),
        [
            "1:agent:request_changes:rename it",
            "2:human:request_changes:agreed, and fix the look"
        ]
    );
}

#[test]
fn the_actions_a_card_allows_come_from_the_daemon() {
    let (_dir, tok) = setup();
    let ready = tok.run(&["show", "DEMO-1"]);
    assert_eq!(
        ready["allowed_actions"],
        serde_json::json!(["start", "pause", "cancel"])
    );
    let h = hand_in(&tok, "DEMO-1");
    assert_eq!(
        h["allowed_actions"],
        serde_json::json!(["accept", "return", "cancel"])
    );
}

#[test]
fn a_review_mode_needs_something_to_set_and_a_known_mode() {
    let (_dir, tok) = setup();
    assert!(tok.fails(&["review-mode"]).contains("give a mode"));
    assert!(
        !tok.fails(&["review-mode", "robot", "--project", "DEMO"])
            .is_empty()
    );
    let p = tok.run(&["review-mode", "--reviewer", "strict", "--project", "DEMO"]);
    assert_eq!(p["default_reviewer"], "strict");
    assert_eq!(p["default_review_mode"], "human", "kept");
}
