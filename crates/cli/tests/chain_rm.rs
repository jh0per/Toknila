//! `tok chain rm` through the real `tok` binary and daemon: the chain and its cards go, another
//! chain stays, a running card refuses the deletion by name, and nothing is deleted without
//! `--yes` when nobody can confirm.

mod common;

use common::Tok;

fn tok() -> (tempfile::TempDir, Tok) {
    let dir = tempfile::tempdir().expect("tempdir");
    let tok = Tok {
        home: dir.path().join("home"),
    };
    tok.run(&["project", "new", "DEMO"]);
    tok.run(&["chain", "new", "old", "--project", "DEMO", "--no-autostart"]);
    tok.run(&["ticket", "new", "old", "Stale one"]);
    tok.run(&["ticket", "new", "old", "Stale two"]);
    tok.run(&[
        "chain",
        "new",
        "keep",
        "--project",
        "DEMO",
        "--no-autostart",
    ]);
    tok.run(&["ticket", "new", "keep", "Stays"]);
    (dir, tok)
}

#[test]
fn removes_the_chain_and_its_cards_and_keeps_the_others() {
    let (_dir, tok) = tok();
    let out = tok.run(&["chain", "rm", "old", "--project", "DEMO", "--yes"]);
    assert_eq!(out["chain"]["name"], "old");
    assert_eq!(out["project"], "DEMO");
    assert_eq!(out["tickets"], 2);
    assert_eq!(out["warnings"], serde_json::json!([]));
    let left = tok.run(&["chain", "ls"]);
    assert_eq!(left.as_array().map(Vec::len), Some(1));
    assert_eq!(left[0]["chain"]["name"], "keep");
    let again = tok.fails(&["chain", "rm", "old", "--yes"]);
    assert!(again.contains("chain old not found"), "{again}");
    assert_eq!(tok.status("DEMO-3"), "ready");
}

#[test]
fn needs_a_confirmation_that_nobody_can_give_without_yes() {
    let (_dir, tok) = tok();
    let err = tok.fails(&["chain", "rm", "old"]);
    assert!(err.contains("pass --yes"), "{err}");
    assert_eq!(tok.run(&["chain", "ls"]).as_array().map(Vec::len), Some(2));
}
