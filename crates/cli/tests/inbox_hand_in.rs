//! `tok inbox` through the real binary and daemon: a card in review shows the hand-in's gist and
//! its checks, and the JSON carries the whole hand-in.

mod common;

use common::Tok;

const CHAIN: &str = r#"
[chain]
name = "demo"
goal = "Inbox"
autostart = false

[[ticket]]
id = "A"
title = "Alpha"
profile = "sim"
"#;

#[test]
fn inbox_prints_the_gist_and_the_checks_of_a_review_item() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tok = Tok::in_dir(dir.path());
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
    tok.run(&["review-mode", "human", "--project", "DEMO"]);
    tok.run(&["start", "DEMO-1"]);
    tok.run(&[
        "sim",
        "DEMO-1",
        "finish",
        "**What changed:** Alpha is built\n**Checks:**\n- PASS verify: test result: ok. 120 passed; 0 failed\n- FAIL e2e: red\n- PASS unit: test result: ok. 3 passed; 0 failed\n**Left:** docs",
    ]);

    let out = tok.text(&["inbox"]);
    assert_eq!(
        out,
        "review  DEMO-1  Alpha\n  gist: Alpha is built\n  checks: PASS 2, FAIL 1\n"
    );

    let json = tok.run(&["inbox"]);
    let hand_in = &json[0]["hand_in"];
    assert_eq!(hand_in["gist"], "Alpha is built");
    assert_eq!(hand_in["left"], "docs");
    assert_eq!(hand_in["checks"][1]["status"], "FAIL");
    assert_eq!(hand_in["checks"][1]["text"], "e2e: red");
}
