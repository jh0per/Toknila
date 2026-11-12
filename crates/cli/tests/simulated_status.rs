//! The simulated agent through the real `tok` binary and daemon: a card under it reads
//! `simulated`, never a plain `running`; a project made without a profile runs the real agent;
//! and a card that gets `sim` from a project not marked for simulation is refused.

mod common;

use common::Tok;

#[test]
fn a_card_under_the_simulated_agent_reads_simulated_in_ls_and_show() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    tok.run(&["project", "new", "DEMO", "--sim"]);
    tok.run(&["chain", "new", "demo", "--goal", "g"]);
    tok.run(&["ticket", "new", "demo", "A"]);
    assert_eq!(
        tok.status("DEMO-1"),
        "running",
        "the wire status is as before"
    );

    let ls = tok.text(&["ls"]);
    let row = ls.lines().find(|l| l.starts_with("DEMO-1")).expect("row");
    assert!(row.contains("simulated"), "{ls}");
    assert!(!row.contains("running"), "{ls}");
    let show = tok.text(&["show", "DEMO-1"]);
    let first = show.lines().next().expect("first line");
    assert!(first.contains("[simulated"), "{show}");
    assert!(!first.contains("running"), "{show}");
}

#[test]
fn a_project_without_a_profile_runs_the_real_agent() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    tok.run(&["project", "new", "REAL"]);
    let shown = tok.run(&["project", "show", "REAL"]);
    assert_eq!(shown["default_profile"], "claude-code");
    assert!(shown.get("simulated").is_none(), "{shown}");
}

#[test]
fn a_card_that_gets_sim_from_a_project_not_marked_for_simulation_is_refused() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    // `sim` by name, but nobody marked the project for simulation (what an old project is).
    tok.run(&["project", "new", "OLD", "--profile", "sim"]);
    tok.run(&["chain", "new", "work", "--project", "OLD"]);
    let msg = tok.fails(&["ticket", "new", "work", "A"]);
    assert!(msg.contains("not marked for simulation"), "{msg}");
}
