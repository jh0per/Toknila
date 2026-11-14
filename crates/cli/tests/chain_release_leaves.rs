//! `tok chain show` through the real `tok` binary and daemon: the epic's release is listed after
//! the stories nothing else waits for, also for stories added after the epic was made.

mod common;

use common::Tok;

/// The AFTER column of the release row of `tok chain show`.
fn release_after(tok: &Tok) -> String {
    let shown = tok.text(&["chain", "show", "phase"]);
    let row = shown
        .lines()
        .find(|l| l.starts_with("<> 0.3.3"))
        .unwrap_or_else(|| panic!("no release row in:\n{shown}"));
    row.split_whitespace().last().expect("a cell").to_owned()
}

#[test]
fn the_release_row_lists_the_last_story_after_tok_ticket_new() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    tok.run(&["project", "new", "DEMO", "--sim"]);
    let made = tok.run(&[
        "chain",
        "new",
        "phase",
        "--project",
        "DEMO",
        "--no-autostart",
        "--version",
        "0.3.3",
    ]);
    let id = made["chain"]["id"].as_str().expect("id").to_owned();
    tok.run(&["ticket", "new", &id, "First"]);
    assert_eq!(release_after(&tok), "DEMO-1");

    tok.run(&["ticket", "new", &id, "Second", "--after", "DEMO-1"]);
    assert_eq!(
        release_after(&tok),
        "DEMO-2",
        "the story a new story depends on leaves the release's list"
    );
}
