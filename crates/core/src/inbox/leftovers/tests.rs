//! The three leftover actions as pure operations on a checklist and a graph.

use super::*;
use crate::graph::ChainView;
use crate::{
    Chain, ChainId, Edge, EdgeKind, NodeRef, OutOfOrder, ProjectId, SubtaskId, TicketStatus,
    Timestamp,
};

fn ticket(n: u32, status: TicketStatus) -> Ticket {
    Ticket::new_backlog(
        TicketId::new(),
        ChainId::new(),
        format!("DEMO-{n}").parse().expect("key"),
        crate::TicketDraft {
            title: format!("Setup {n}"),
            ..crate::TicketDraft::default()
        },
        Timestamp(1),
    )
    .with_status(status)
}

trait WithStatus {
    fn with_status(self, s: TicketStatus) -> Self;
}

impl WithStatus for Ticket {
    fn with_status(mut self, s: TicketStatus) -> Self {
        self.status = s;
        self
    }
}

fn sub(card: &Ticket, position: u32, text: &str, done: bool) -> Subtask {
    Subtask {
        id: SubtaskId::new(),
        ticket_id: card.id,
        position,
        text: text.to_owned(),
        done,
        done_by: done.then_some(Actor::Human),
    }
}

fn checklist(card: &Ticket) -> Vec<Subtask> {
    vec![
        sub(card, 1, "T001 done", true),
        sub(card, 2, "T002  open  one", false),
        sub(card, 3, "T003 also done", true),
        sub(card, 4, "T004 open two", false),
    ]
}

#[test]
fn open_subtasks_are_the_undone_ones_of_that_card_in_order() {
    let a = ticket(1, TicketStatus::Ready);
    let b = ticket(2, TicketStatus::Ready);
    let mut all = checklist(&a);
    all.push(sub(&b, 1, "other card", false));
    let open: Vec<_> = open_subtasks(&all, a.id)
        .into_iter()
        .map(|s| s.text)
        .collect();
    assert_eq!(open, ["T002  open  one", "T004 open two"]);
}

#[test]
fn moving_takes_the_open_subtasks_verbatim_and_renumbers_what_stays() {
    let a = ticket(1, TicketStatus::Ready);
    let new_card = TicketId::new();
    let before = checklist(&a);
    let m = move_out(&before, new_card);
    let moved: Vec<_> = m
        .moved
        .iter()
        .map(|s| (s.ticket_id, s.position, s.text.as_str(), s.done))
        .collect();
    assert_eq!(
        moved,
        [
            (new_card, 1, "T002  open  one", false),
            (new_card, 2, "T004 open two", false)
        ]
    );
    let kept: Vec<_> = m
        .kept
        .iter()
        .map(|s| (s.ticket_id, s.position, s.text.as_str()))
        .collect();
    assert_eq!(kept, [(a.id, 1, "T001 done"), (a.id, 2, "T003 also done")]);
    // The same rows move, so their ids stay.
    assert_eq!((m.moved[0].id, m.moved[1].id), (before[1].id, before[3].id));
}

#[test]
fn closing_marks_only_the_open_subtasks_done_by_the_person() {
    let a = ticket(1, TicketStatus::Ready);
    let closed = close(&checklist(&a), Actor::Human);
    let got: Vec<_> = closed
        .iter()
        .map(|s| (s.position, s.done, s.done_by))
        .collect();
    assert_eq!(
        got,
        [(2, true, Some(Actor::Human)), (4, true, Some(Actor::Human))]
    );
}

#[test]
fn the_new_card_is_named_after_the_old_one() {
    let a = ticket(1, TicketStatus::Ready);
    assert_eq!(new_card_title(&a), "Leftovers from Setup 1");
    assert!(
        new_card_prompt(&a).contains("DEMO-1"),
        "{}",
        new_card_prompt(&a)
    );
}

#[test]
fn notes_say_what_was_closed_or_moved() {
    let a = ticket(1, TicketStatus::Ready);
    let closed = close(&checklist(&a), Actor::Human);
    let note = closing_note(&a, &closed);
    assert_eq!(note.gist, "Closed 2 leftover subtasks of DEMO-1");
    assert_eq!(
        note.what.as_deref(),
        Some("- T002  open  one\n- T004 open two")
    );
    let to: TicketKey = "DEMO-9".parse().expect("key");
    let moved = move_out(&checklist(&a), TicketId::new()).moved;
    assert_eq!(
        moving_note(&a, &to, &moved).gist,
        "Moved 2 leftover subtasks of DEMO-1 to DEMO-9"
    );
    assert_eq!(
        closing_note(&a, &closed.into_iter().take(1).collect::<Vec<_>>()).gist,
        "Closed 1 leftover subtask of DEMO-1"
    );
}

fn view(tickets: Vec<Ticket>, edges: &[(usize, usize)]) -> ChainView {
    let chain = Chain {
        id: ChainId::new(),
        project_id: ProjectId::new(),
        name: "c".to_owned(),
        goal: String::new(),
        autostart: false,
        review_mode: None,
        reviewer: None,
        source: None,
        version: None,
    };
    let edges = edges
        .iter()
        .map(|(f, t)| Edge {
            chain_id: chain.id,
            from: NodeRef::Ticket(tickets[*f].id),
            to: NodeRef::Ticket(tickets[*t].id),
            kind: EdgeKind::Normal,
        })
        .collect();
    ChainView {
        chain,
        tickets,
        releases: vec![],
        edges,
        release_tickets: vec![],
    }
}

#[test]
fn the_end_of_the_chain_is_every_ticket_nothing_waits_for() {
    // 0 -> 1 -> 3, 0 -> 2: the ends are 2 and 3.
    let t: Vec<Ticket> = (1..=4).map(|n| ticket(n, TicketStatus::Ready)).collect();
    let ids: Vec<_> = t.iter().map(|t| t.id).collect();
    let v = view(t, &[(0, 1), (1, 3), (0, 2)]);
    assert_eq!(chain_end(&v), [ids[2], ids[3]]);
}

#[test]
fn a_proposal_that_is_not_approved_is_not_the_end_of_the_chain() {
    let mut t: Vec<Ticket> = (1..=2).map(|n| ticket(n, TicketStatus::Ready)).collect();
    t[1] = ticket(2, TicketStatus::Proposed);
    t[1].wildcard = Some(crate::Wildcard {
        from: None,
        proposed_by: Actor::Agent,
        after: vec![],
        before: vec![],
        approved: false,
    });
    let first = t[0].id;
    assert_eq!(chain_end(&view(t, &[])), [first]);
}

#[test]
fn only_a_card_with_an_undismissed_mark_and_open_subtasks_has_leftovers_to_decide() {
    let a = ticket(1, TicketStatus::Ready);
    let mut b = ticket(2, TicketStatus::Merged);
    b.done_out_of_order = Some(OutOfOrder::new(vec![a.key.clone()]));
    let subs = checklist(&a);
    let all = vec![a.clone(), b.clone()];
    assert_eq!(require_pending(&all, &subs, &a), Ok(()));
    // No open subtasks.
    assert_eq!(
        require_pending(&all, &[], &a),
        Err(LeftoverError::NothingLeft(a.key.clone()))
    );
    // Dismissed.
    let mut dismissed = all.clone();
    dismissed[1]
        .done_out_of_order
        .as_mut()
        .expect("marked")
        .dismiss(&a.key);
    assert_eq!(
        require_pending(&dismissed, &subs, &a),
        Err(LeftoverError::NothingLeft(a.key.clone()))
    );
    // Not behind anything.
    assert_eq!(
        require_pending(std::slice::from_ref(&a), &subs, &a),
        Err(LeftoverError::NothingLeft(a.key.clone()))
    );
}
