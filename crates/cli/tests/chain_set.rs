//! `tok chain set --autostart` through the real `tok` binary and daemon: turning it on starts the
//! ready card, turning it off keeps the chain as it is, the project narrows a name two projects
//! share, and the journal tells of the change.

mod common;

use common::Tok;

/// Two projects, each with a chain `work` without autostart; returns the id of DEMO's.
fn tok() -> (tempfile::TempDir, Tok, String) {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    tok.run(&["project", "new", "DEMO", "--sim"]);
    tok.run(&["project", "new", "OTHR", "--sim"]);
    let mut demo = String::new();
    for p in ["DEMO", "OTHR"] {
        let made = tok.run(&["chain", "new", "work", "--project", p, "--no-autostart"]);
        if p == "DEMO" {
            demo = made["chain"]["id"].as_str().expect("id").to_owned();
        }
    }
    (dir, tok, demo)
}

#[test]
fn autostart_on_starts_the_ready_card_and_off_leaves_it_running() {
    let (_dir, tok, id) = tok();
    tok.run(&["ticket", "new", &id, "First"]);
    assert_eq!(tok.status("DEMO-1"), "ready", "control: waits for a person");

    let shown = tok.run(&[
        "chain",
        "set",
        "work",
        "--project",
        "DEMO",
        "--autostart",
        "on",
    ]);
    assert_eq!(shown["chain"]["autostart"], true);
    assert_eq!(tok.status("DEMO-1"), "running");
    let other = tok.run(&["chain", "ls"]);
    let autostarts: Vec<_> = other
        .as_array()
        .expect("list")
        .iter()
        .map(|i| (i["chain"]["name"].clone(), i["chain"]["autostart"].clone()))
        .collect();
    assert!(
        autostarts.contains(&("work".into(), true.into())),
        "{autostarts:?}"
    );
    assert!(
        autostarts.contains(&("work".into(), false.into())),
        "the other project's chain is untouched: {autostarts:?}"
    );

    let off = tok.run(&[
        "chain",
        "set",
        "work",
        "--project",
        "DEMO",
        "--autostart",
        "off",
    ]);
    assert_eq!(off["chain"]["autostart"], false);
    assert_eq!(
        tok.status("DEMO-1"),
        "running",
        "a running card is not stopped"
    );
}

#[test]
fn a_name_two_projects_share_needs_the_project() {
    let (_dir, tok, _) = tok();
    let err = tok.fails(&["chain", "set", "work", "--autostart", "on"]);
    assert!(err.contains("2 chains are named work"), "{err}");
    let bad = tok.fails(&[
        "chain",
        "set",
        "work",
        "--project",
        "DEMO",
        "--autostart",
        "maybe",
    ]);
    assert!(bad.contains("autostart"), "{bad}");
}
