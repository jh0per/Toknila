//! The chain as an epic: the goal reaches the card's agent, the version makes and renames the end
//! release, and a release that was cut keeps its version.

use toknila_core::{AgentRole, ReleaseState};

use super::Mem;
use super::runs::{settle, with_agents};
use crate::chains::{self, ChainSettings, NewChain};
use crate::recovery::recover;

fn settings(version: &str) -> ChainSettings {
    ChainSettings {
        version: Some(version.to_owned()),
        ..ChainSettings::default()
    }
}

#[test]
fn the_goal_of_the_chain_is_in_the_brief_of_every_card_agent() {
    let mut mem = with_agents();
    mem.view.chain.goal = "Pay with saved cards.".to_owned();
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    let started = mem.runner.started.borrow();
    let brief = &started
        .iter()
        .find(|r| r.role == AgentRole::Worker)
        .expect("a worker started")
        .brief;
    assert_eq!(
        brief.epic,
        Some(("demo".to_owned(), "Pay with saved cards.".to_owned()))
    );
}

#[test]
fn a_version_makes_an_end_release_that_waits_for_the_cards_and_a_new_version_renames_it() {
    let mut mem = Mem::demo();
    assert_eq!(mem.view.version(), None, "control: a chain without one");
    chains::set_settings(&mut mem, "demo", None, settings("v0.3.0")).expect("version");
    assert_eq!(mem.view.chain.version.as_deref(), Some("0.3.0"));
    assert_eq!(mem.view.releases.len(), 1);
    assert_eq!(mem.view.releases[0].version, "0.3.0");
    assert_eq!(
        mem.view
            .edges
            .iter()
            .filter(|e| e.to == toknila_core::NodeRef::Release(mem.view.releases[0].id))
            .count(),
        1,
        "it waits for the last card"
    );
    chains::set_settings(&mut mem, "demo", None, settings("0.4.0")).expect("rename");
    assert_eq!(mem.view.releases.len(), 1, "renamed, not added");
    assert_eq!(mem.view.releases[0].version, "0.4.0");
    assert_eq!(mem.view.chain.version.as_deref(), Some("0.4.0"));
}

#[test]
fn a_cut_release_keeps_its_version_and_a_bad_one_is_refused() {
    let mut mem = Mem::demo();
    chains::set_settings(&mut mem, "demo", None, settings("0.3.0")).expect("version");
    let bad = chains::set_settings(&mut mem, "demo", None, settings("soon")).expect_err("bad");
    assert!(
        bad.to_string().contains("numbers separated by dots"),
        "{bad}"
    );
    mem.view.releases[0].state = ReleaseState::Cut;
    let err = chains::set_settings(&mut mem, "demo", None, settings("0.4.0")).expect_err("cut");
    assert!(err.to_string().contains("being released"), "{err}");
    assert_eq!(mem.view.releases[0].version, "0.3.0");
    // Saying the version it already has is no change.
    chains::set_settings(&mut mem, "demo", None, settings("0.3.0")).expect("same");
}

#[test]
fn a_new_chain_with_a_version_is_made_with_its_end_release() {
    let mut mem = Mem::demo();
    let input = NewChain {
        name: "epic".to_owned(),
        goal: None,
        autostart: None,
        review_mode: None,
        reviewer: None,
        version: Some("1.0.0".to_owned()),
    };
    chains::create(&mut mem, "DEMO", input).expect("create");
    assert_eq!(mem.view.chain.version.as_deref(), Some("1.0.0"));
    assert_eq!(mem.view.releases.len(), 1);
    assert_eq!(mem.view.releases[0].version, "1.0.0");
    assert_eq!(
        mem.view.finish().map(|f| f.version).as_deref(),
        Some("1.0.0")
    );
}
