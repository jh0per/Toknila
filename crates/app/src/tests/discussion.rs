//! A card's discussion with the assistant through the use cases: only a person talks and decides,
//! and the card's agent starts with the discussion in its brief.

use toknila_core::Actor;

use super::Mem;
use super::runs::with_agents;
use crate::external::ExternalEffect;
use crate::recovery::recover;
use crate::{ErrorKind, discussion};

#[test]
fn only_a_person_talks_to_the_assistant() {
    let mut mem = Mem::demo();
    mem.actor = Actor::Agent;
    let a = mem.ticket_by_title("A");
    let err = discussion::say(&mut mem, &a, "hello").expect_err("an agent cannot");
    assert_eq!(err.kind, ErrorKind::Conflict, "{err}");
    assert!(discussion::find(&mem, &a).expect("find").is_none());
}

#[test]
fn the_card_agent_starts_with_the_discussion_in_its_brief_and_a_pending_edit_is_marked() {
    let mut mem = with_agents();
    let a = mem.ticket_by_title("A");
    discussion::say(&mut mem, &a, "make it stricter").expect("say");
    let (th, _) = discussion::say(&mut mem, &a, "and shorter").expect("say again");
    mem.actor = Actor::Agent;
    let proposal = toknila_core::discussion::EditProposal::of(
        &a.prompt,
        &a.acceptance,
        Some("a stricter prompt".to_owned()),
        None,
    );
    discussion::propose(&mut mem, &a, &th, &proposal).expect("propose");

    recover(&mut mem).expect("settle");
    let brief = mem
        .deferred
        .iter()
        .find_map(|e| match e {
            ExternalEffect::StartRunner { ticket, brief, .. } if *ticket == a.id => {
                Some(brief.clone())
            }
            _ => None,
        })
        .expect("the card's runner is queued");
    assert_eq!(
        brief.discussion,
        vec![
            "person: make it stricter".to_owned(),
            "person: and shorter".to_owned(),
            "assistant proposed an edit (waiting for the person); prompt to: a stricter prompt"
                .to_owned(),
        ]
    );
}
