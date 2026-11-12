//! "Design first" through the real `tok` binary and daemon: `tok ticket new --design-first`,
//! `tok design`, and the hand-in gate with a simulated agent.

mod common;

use common::Tok;

fn setup() -> (tempfile::TempDir, Tok) {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    tok.run(&["project", "new", "DEMO", "--name", "Demo", "--sim"]);
    tok.run(&[
        "chain",
        "new",
        "demo",
        "--project",
        "DEMO",
        "--no-autostart",
    ]);
    (dir, tok)
}

#[test]
fn a_design_can_be_asked_for_when_the_card_is_made_or_later() {
    let (_dir, tok) = setup();
    let made = tok.run(&[
        "ticket",
        "new",
        "demo",
        "Login screen",
        "--profile",
        "sim",
        "--design-first",
    ]);
    assert_eq!(made["ticket"]["design_first"], true, "{made}");
    tok.run(&["ticket", "new", "demo", "Plain", "--profile", "sim"]);
    let asked = tok.run(&["design", "DEMO-2"]);
    assert_eq!(asked["ticket"]["design_first"], true, "{asked}");
    let shown = tok.run(&["show", "DEMO-2"]);
    assert!(
        !shown["allowed_actions"]
            .as_array()
            .expect("actions")
            .iter()
            .any(|a| a == "ask_design"),
        "not offered twice: {shown}"
    );
}

#[test]
fn the_hand_in_waits_for_an_approved_design() {
    let (_dir, tok) = setup();
    tok.run(&[
        "ticket",
        "new",
        "demo",
        "Login screen",
        "--profile",
        "sim",
        "--design-first",
    ]);
    tok.run(&["start", "DEMO-1"]);
    let refused = tok.fails(&["sim", "DEMO-1", "finish", "done"]);
    assert!(refused.contains("open_thread"), "{refused}");
    assert_eq!(tok.run(&["show", "DEMO-1"])["ticket"]["status"], "running");
}
