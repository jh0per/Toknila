//! Wildcard cards on the graph: proposed work stays out of the plan until a person approves it.

use super::*;
use crate::events::DomainEvent;
use crate::{Actor, TicketDraft, Wildcard};

impl Builder {
    /// Proposes a wildcard card found during DEMO-1 with these pending dependencies.
    fn card(&mut self, after: &[NodeRef], before: &[NodeRef]) -> NodeRef {
        let mark = Wildcard {
            from: Some("DEMO-1".parse().expect("key")),
            proposed_by: Actor::Agent,
            after: after.to_vec(),
            before: before.to_vec(),
            approved: false,
        };
        let draft = TicketDraft {
            title: format!("w{}", self.next),
            profile: Some("sim".to_owned()),
            ..TicketDraft::default()
        };
        let key = format!("DEMO-{}", self.next).parse().expect("key");
        let card = Ticket::new_proposed(TicketId::new(), self.view.chain.id, key, draft, mark, T);
        self.next += 1;
        let id = card.id;
        self.view.propose(card).expect("valid proposal");
        NodeRef::Ticket(id)
    }
}

fn approve(v: &mut ChainView, n: NodeRef) -> Result<Vec<Effect>, GraphError> {
    let effects = v.apply_ticket_event(tid(n), &TicketEvent::Approve { by: Actor::Human }, T)?;
    settle(v);
    Ok(effects)
}

fn has_edge(v: &ChainView, from: NodeRef, to: NodeRef) -> bool {
    v.edges.iter().any(|e| e.from == from && e.to == to)
}

/// A running, B waits for A.
fn a_then_b() -> (Builder, NodeRef, NodeRef) {
    let mut b = Builder::new(true);
    let (a, bb) = (b.ticket(), b.ticket());
    b.edge(a, bb);
    settle(&mut b.view);
    (b, a, bb)
}

#[test]
fn a_proposed_card_never_starts_and_changes_no_readiness() {
    let (mut b, a, bb) = a_then_b();
    let p = b.card(&[], &[bb]);
    let mut v = b.view;
    settle(&mut v);
    assert_eq!(
        status(&v, p),
        TicketStatus::Proposed,
        "no dependencies, still never starts"
    );
    assert!(!v.edges.iter().any(|e| e.from == p || e.to == p));
    finish_and_accept(&mut v, a);
    assert_eq!(
        status(&v, bb),
        TicketStatus::Running,
        "B does not wait for a proposal"
    );
    assert_eq!(status(&v, p), TicketStatus::Proposed);
}

#[test]
fn a_proposed_card_does_not_count_in_progress_but_needs_a_human() {
    let (mut b, a, bb) = a_then_b();
    let p = b.card(&[a], &[]);
    let mut v = b.view;
    let s = v.summary(T);
    assert_eq!((s.total, s.done), (2, 0));
    assert_eq!(
        s.status,
        ChainStatus::NeedsHuman,
        "a decision waits on a person"
    );
    finish_and_accept(&mut v, a);
    finish_and_accept(&mut v, bb);
    let s = v.summary(T);
    assert_eq!((s.total, s.done, s.status), (2, 2, ChainStatus::NeedsHuman));
    let stop = s.stopped_at.expect("the card is where a person is needed");
    assert_eq!(
        (stop.ticket.to_string(), stop.status, stop.reason),
        (
            "DEMO-3".to_owned(),
            TicketStatus::Proposed,
            Some(StatusReason::Proposed)
        )
    );
    let reject = TicketEvent::Reject {
        reason: "not needed".to_owned(),
        by: Actor::Human,
    };
    event(&mut v, p, reject);
    let s = v.summary(T);
    assert_eq!(
        (s.total, s.done, s.status),
        (2, 2, ChainStatus::Done),
        "a rejected card never joined the plan"
    );
}

#[test]
fn a_chain_with_only_a_proposal_needs_a_human() {
    let mut b = Builder::new(true);
    b.card(&[], &[]);
    let s = b.view.summary(T);
    assert_eq!((s.total, s.status), (0, ChainStatus::NeedsHuman));
}

#[test]
fn approval_adds_the_proposed_edges_and_the_graph_decides_the_rest() {
    let (mut b, a, bb) = a_then_b();
    let p = b.card(&[a], &[bb]);
    let mut v = b.view;
    let effects = approve(&mut v, p).expect("approve");
    assert_eq!(
        effects,
        [Effect::AddProposedEdges {
            after: vec![a],
            before: vec![bb]
        }]
    );
    assert!(has_edge(&v, a, p) && has_edge(&v, p, bb));
    assert_eq!(status(&v, p), TicketStatus::Backlog, "waits for A");
    let mark = v
        .ticket(tid(p))
        .and_then(|t| t.wildcard.clone())
        .expect("mark");
    assert!(mark.approved && mark.after.is_empty() && mark.before.is_empty());
    assert_eq!(
        v.summary(T).total,
        3,
        "an approved card is part of the plan"
    );
    finish_and_accept(&mut v, a);
    assert_eq!(
        [status(&v, p), status(&v, bb)],
        [TicketStatus::Running, TicketStatus::Backlog],
        "B now waits for the card"
    );
    finish_and_accept(&mut v, p);
    assert_eq!(status(&v, bb), TicketStatus::Running);
}

#[test]
fn an_approved_card_without_dependencies_autostarts() {
    let mut b = Builder::new(true);
    let p = b.card(&[], &[]);
    let mut v = b.view;
    approve(&mut v, p).expect("approve");
    assert_eq!(status(&v, p), TicketStatus::Running);
}

#[test]
fn a_ready_target_goes_back_to_backlog_when_a_card_goes_before_it() {
    let mut b = Builder::new(false);
    let a = b.ticket();
    settle(&mut b.view);
    assert_eq!(status(&b.view, a), TicketStatus::Ready);
    let p = b.card(&[], &[a]);
    let mut v = b.view;
    approve(&mut v, p).expect("approve");
    assert_eq!(
        [status(&v, p), status(&v, a)],
        [TicketStatus::Ready, TicketStatus::Backlog]
    );
}

#[test]
fn approval_with_a_cycle_fails_and_the_card_stays_proposed() {
    let mut b = Builder::new(true);
    let (a, bb, c) = (b.ticket(), b.ticket(), b.ticket());
    b.edge(a, bb);
    b.edge(bb, c);
    settle(&mut b.view);
    let p = b.card(&[c], &[bb]);
    let mut v = b.view;
    let before = v.clone();
    let err = approve(&mut v, p).expect_err("cycle");
    assert_eq!(
        err.to_string(),
        "dependency cycle: DEMO-2 -> DEMO-3 -> DEMO-4 -> DEMO-2"
    );
    assert_eq!(v, before, "nothing changed: the card is still proposed");
    assert_eq!(status(&v, p), TicketStatus::Proposed);
}

#[test]
fn approval_before_started_work_fails_and_names_it() {
    let (mut b, a, _) = a_then_b();
    let r = b.release("R1", StartMode::Manual);
    b.view.cut_release(rid(r), T).expect("cut");
    let p = b.card(&[], &[a]);
    let q = b.card(&[], &[r]);
    let mut v = b.view;
    let before = v.clone();
    let err = approve(&mut v, p).expect_err("A runs");
    assert_eq!(
        err,
        GraphError::BeforeStarted {
            node: "DEMO-1".to_owned(),
            state: "running".to_owned()
        }
    );
    assert_eq!(
        err.to_string(),
        "DEMO-1 is already running; a card can only go before work that has not started"
    );
    let err = approve(&mut v, q).expect_err("R1 is cut");
    assert_eq!(
        err.to_string(),
        "release R1 is already cut; a card can only go before work that has not started"
    );
    assert_eq!(v, before);
}

#[test]
fn nothing_links_to_a_card_until_a_person_decides() {
    let (mut b, a, bb) = a_then_b();
    let p = b.card(&[], &[]);
    let q = b.card(&[p], &[]);
    let chain_id = b.view.chain.id;
    let mut v = b.view;
    let edge = Edge {
        chain_id,
        from: p,
        to: bb,
        kind: EdgeKind::Normal,
    };
    let still = GraphError::StillProposed("DEMO-3".to_owned());
    assert_eq!(v.add_edge(edge), Err(still.clone()));
    assert_eq!(
        still.to_string(),
        "DEMO-3 is a proposed card; approve or reject it first"
    );
    let into = Edge {
        from: a,
        to: p,
        ..edge
    };
    assert_eq!(v.add_edge(into), Err(still.clone()));
    assert_eq!(approve(&mut v, q), Err(still), "q waits for a proposal");
    approve(&mut v, p).expect("approve p");
    approve(&mut v, q).expect("now q can follow p");
    assert!(has_edge(&v, p, q));
}

#[test]
fn a_proposal_must_be_a_proposed_card_on_known_nodes() {
    let mut b = Builder::new(true);
    let a = b.ticket();
    let mut planned = b.view.tickets[0].clone();
    planned.id = TicketId::new();
    assert_eq!(
        b.view.propose(planned),
        Err(GraphError::NotProposed("DEMO-1".to_owned()))
    );
    let ghost = NodeRef::Ticket(TicketId::new());
    let mark = Wildcard {
        from: None,
        proposed_by: Actor::Human,
        after: vec![a, ghost],
        before: vec![],
        approved: false,
    };
    let key = "DEMO-2".parse().expect("key");
    let card = Ticket::new_proposed(
        TicketId::new(),
        b.view.chain.id,
        key,
        TicketDraft::default(),
        mark,
        T,
    );
    assert_eq!(b.view.propose(card), Err(GraphError::UnknownNode(ghost)));
    assert_eq!(b.view.tickets.len(), 1);
}

#[test]
fn decisions_are_journaled_with_their_edges() {
    let (mut b, a, bb) = a_then_b();
    let p = b.card(&[a], &[bb]);
    let q = b.card(&[], &[]);
    let mut v = b.view;
    let (_, events) = v
        .record(|v| v.apply_ticket_event(tid(p), &TicketEvent::Approve { by: Actor::Human }, T))
        .expect("approve");
    let kinds: Vec<String> = events
        .iter()
        .map(|e| format!("{}.{}", e.entity(), e.kind()))
        .collect();
    assert_eq!(
        kinds,
        [
            "ticket.approved",
            "ticket.status_changed",
            "chain.edge_added",
            "chain.edge_added"
        ]
    );
    let reject = TicketEvent::Reject {
        reason: "later".to_owned(),
        by: Actor::Human,
    };
    let (_, events) = v
        .record(|v| v.apply_ticket_event(tid(q), &reject, T))
        .expect("reject");
    match &events[..] {
        [
            DomainEvent::TicketRejected { key, reason, .. },
            DomainEvent::TicketStatusChanged { from, to, .. },
        ] => {
            assert_eq!(
                (key.to_string(), reason.as_str(), *from, *to),
                (
                    "DEMO-4".to_owned(),
                    "later",
                    TicketStatus::Proposed,
                    TicketStatus::Cancelled
                )
            );
        }
        other => panic!("unexpected events {other:?}"),
    }
}
