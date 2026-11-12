//! Wildcard cards through the use cases: an agent or a person proposes, only a person decides.

use toknila_core::agent::Proposal;
use toknila_core::events::DomainEvent;
use toknila_core::machine::TicketEvent;
use toknila_core::{Actor, NodeRef, StatusReason, TicketId, TicketStatus};

use super::Mem;
use crate::recovery::recover;
use crate::tickets::{self, TicketAction};
use crate::{AppError, ErrorKind, agent, wildcards};

fn proposal(title: &str, after: &[&str], before: &[&str]) -> Proposal {
    Proposal {
        title: title.to_owned(),
        prompt: "add the endpoint".to_owned(),
        acceptance: vec!["401 refreshes the token".to_owned()],
        after: after.iter().map(|s| (*s).to_owned()).collect(),
        before: before.iter().map(|s| (*s).to_owned()).collect(),
    }
}

/// DEMO-1 (A) running, DEMO-2 (B) waiting for it.
fn running_demo() -> Mem {
    let mut mem = Mem::demo();
    recover(&mut mem).expect("settle");
    mem.events.clear();
    mem
}

fn decide(
    mem: &mut Mem,
    card: TicketId,
    action: TicketAction,
    text: Option<&str>,
) -> Result<(), AppError> {
    let t = mem.ticket_by_id(card);
    let event = action.event(text.map(str::to_owned), mem.actor)?;
    tickets::act(mem, &t, event)
}

#[test]
fn an_agent_proposes_a_card_found_during_its_ticket() {
    let mut mem = running_demo();
    mem.actor = Actor::Agent;
    let a = mem.ticket_by_title("A");
    let event = toknila_core::agent::AgentEvent::Propose(proposal("refresh", &[], &["DEMO-2"]));
    let id = agent::ticket_event(&mut mem, &a, event).expect("propose");
    let card = mem.ticket_by_id(id);
    assert_eq!(card.key.to_string(), "DEMO-3");
    assert_eq!(
        (card.status, card.status_reason.clone()),
        (TicketStatus::Proposed, Some(StatusReason::Proposed))
    );
    assert_eq!(card.acceptance, ["401 refreshes the token"]);
    let mark = card.wildcard.clone().expect("mark");
    let b = mem.ticket_by_title("B");
    assert_eq!(
        (
            mark.from.map(|k| k.to_string()),
            mark.proposed_by,
            mark.before,
            mark.approved
        ),
        (
            Some("DEMO-1".to_owned()),
            Actor::Agent,
            vec![NodeRef::Ticket(b.id)],
            false
        )
    );
    assert_eq!(
        b.status,
        TicketStatus::Backlog,
        "B does not wait for a proposal"
    );
    assert_eq!(mem.kinds(), ["ticket.proposed"]);
    assert!(mem.view.edges.iter().all(|e| e.from != NodeRef::Ticket(id)));
}

#[test]
fn a_person_proposes_and_approves_and_the_edges_follow() {
    let mut mem = running_demo();
    let card = proposal(" refresh ", &["A"], &["DEMO-2"]);
    let id = wildcards::propose(&mut mem, "demo", card, Some("demo-1")).expect("propose");
    let mark = mem.ticket_by_id(id).wildcard.expect("mark");
    assert_eq!(mark.proposed_by, Actor::Human);
    assert_eq!(mem.ticket_by_id(id).title, "refresh");
    mem.events.clear();
    decide(&mut mem, id, TicketAction::Approve, None).expect("approve");
    let card = mem.ticket_by_id(id);
    assert_eq!(card.status, TicketStatus::Backlog, "it waits for A");
    assert!(card.wildcard.expect("the mark stays").approved);
    let (a, b) = (mem.ticket_by_title("A"), mem.ticket_by_title("B"));
    let edges: Vec<_> = mem.view.edges.iter().map(|e| (e.from, e.to)).collect();
    assert!(edges.contains(&(NodeRef::Ticket(a.id), NodeRef::Ticket(id))));
    assert!(edges.contains(&(NodeRef::Ticket(id), NodeRef::Ticket(b.id))));
    assert_eq!(
        mem.kinds(),
        [
            "ticket.approved",
            "ticket.status_changed",
            "chain.edge_added",
            "chain.edge_added"
        ]
    );
}

#[test]
fn approval_that_makes_a_cycle_is_refused_and_changes_nothing() {
    let mut mem = running_demo();
    let id = wildcards::propose(&mut mem, "demo", proposal("loop", &["B"], &["B"]), None)
        .expect("a proposal is not checked for cycles");
    let (before, events) = (mem.view.clone(), mem.events.len());
    let e = decide(&mut mem, id, TicketAction::Approve, None).expect_err("cycle");
    assert_eq!(
        (e.kind, e.message.as_str()),
        (
            ErrorKind::Conflict,
            "dependency cycle: DEMO-2 -> DEMO-3 -> DEMO-2"
        )
    );
    assert_eq!(mem.view, before);
    assert_eq!(mem.events.len(), events);
    assert_eq!(mem.ticket_by_id(id).status, TicketStatus::Proposed);
}

#[test]
fn approval_before_a_started_ticket_is_refused() {
    let mut mem = running_demo();
    let id = wildcards::propose(&mut mem, "demo", proposal("late", &[], &["DEMO-1"]), None)
        .expect("propose");
    let e = decide(&mut mem, id, TicketAction::Approve, None).expect_err("A runs");
    assert_eq!(
        (e.kind, e.message.as_str()),
        (
            ErrorKind::Conflict,
            "DEMO-1 is already running; a story can only go before work that has not started"
        )
    );
    assert_eq!(mem.ticket_by_id(id).status, TicketStatus::Proposed);
}

#[test]
fn reject_needs_a_reason_and_cancels_the_card() {
    let mut mem = running_demo();
    let id =
        wildcards::propose(&mut mem, "demo", proposal("maybe", &[], &[]), None).expect("propose");
    let e = decide(&mut mem, id, TicketAction::Reject, Some(" ")).expect_err("no reason");
    assert_eq!(
        (e.kind, e.message.as_str()),
        (ErrorKind::BadRequest, "reject needs a text")
    );
    decide(&mut mem, id, TicketAction::Reject, Some("out of scope")).expect("reject");
    let card = mem.ticket_by_id(id);
    assert_eq!(
        (card.status, card.status_reason),
        (
            TicketStatus::Cancelled,
            Some(StatusReason::Cancelled {
                text: "out of scope".to_owned()
            })
        )
    );
    assert!(matches!(
        mem.events.iter().rev().nth(1),
        Some(DomainEvent::TicketRejected { reason, .. }) if reason == "out of scope"
    ));
}

#[test]
fn only_a_person_approves_or_rejects() {
    let mut mem = running_demo();
    let id = wildcards::propose(&mut mem, "demo", proposal("x", &[], &[]), None).expect("propose");
    mem.actor = Actor::Agent;
    let e = decide(&mut mem, id, TicketAction::Approve, None).expect_err("agent");
    assert_eq!(
        (e.kind, e.message.as_str()),
        (
            ErrorKind::Conflict,
            "only a person can approve a story; this came from Agent"
        )
    );
    let e = decide(&mut mem, id, TicketAction::Reject, Some("no")).expect_err("agent");
    assert_eq!(e.kind, ErrorKind::Conflict);
    assert_eq!(mem.ticket_by_id(id).status, TicketStatus::Proposed);
    let ticket = mem.ticket_by_id(id);
    let approve = TicketEvent::Approve { by: Actor::Agent };
    assert!(tickets::act(&mut mem, &ticket, approve).is_err());
}

#[test]
fn proposals_are_checked_before_anything_changes() {
    let mut mem = running_demo();
    let blank = wildcards::propose(&mut mem, "demo", proposal(" ", &[], &[]), None);
    assert_eq!(
        blank.map_err(|e| (e.kind, e.message)),
        Err((ErrorKind::BadRequest, "story title is empty".to_owned()))
    );
    let unknown = wildcards::propose(&mut mem, "demo", proposal("x", &["NOPE-9"], &[]), None);
    assert_eq!(unknown.map_err(|e| e.kind), Err(ErrorKind::NotFound));
    assert!(mem.events.is_empty());
    assert_eq!(mem.view.tickets.len(), 2);
}
