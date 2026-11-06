use super::*;
use crate::OutOfOrder;

fn behind() -> Vec<TicketKey> {
    vec!["DEMO-7".parse().expect("key")]
}

#[test]
fn done_out_of_order_merges_a_waiting_card_and_keeps_the_mark() {
    for from in [TicketStatus::Backlog, TicketStatus::Ready] {
        let t = apply(
            &ticket_in(from),
            &TicketEvent::DoneOutOfOrder { behind: behind() },
            NOW,
        )
        .expect("allowed");
        assert_eq!(t.ticket.status, TicketStatus::Merged);
        assert_eq!(t.ticket.status_since, NOW);
        assert_eq!(
            t.ticket.done_out_of_order,
            Some(OutOfOrder { behind: behind() })
        );
        assert_eq!(t.effects, [Effect::DependencySatisfied]);
        assert_eq!(check_invariants(&t.ticket), Ok(()));
    }
}

#[test]
fn done_out_of_order_needs_something_behind_and_a_waiting_card() {
    let ev = TicketEvent::DoneOutOfOrder { behind: vec![] };
    assert_eq!(
        apply(&ticket_in(TicketStatus::Backlog), &ev, NOW),
        Err(TransitionError::NothingBehind)
    );
    for from in [
        TicketStatus::Running,
        TicketStatus::Review,
        TicketStatus::Merged,
    ] {
        let ev = TicketEvent::DoneOutOfOrder { behind: behind() };
        assert!(matches!(
            apply(&ticket_in(from), &ev, NOW),
            Err(TransitionError::NotAllowed { .. })
        ));
    }
}

#[test]
fn a_paused_card_done_out_of_order_is_no_longer_paused() {
    let mut t = ticket_in(TicketStatus::Backlog);
    t.paused = true;
    let ev = TicketEvent::DoneOutOfOrder { behind: behind() };
    let out = apply(&t, &ev, NOW).expect("allowed");
    assert!(!out.ticket.paused);
    assert_eq!(check_invariants(&out.ticket), Ok(()));
}

#[test]
fn the_mark_survives_reopen() {
    let ev = TicketEvent::DoneOutOfOrder { behind: behind() };
    let merged = apply(&ticket_in(TicketStatus::Backlog), &ev, NOW)
        .expect("merged")
        .ticket;
    let reopened = apply(&merged, &TicketEvent::Reopen, NOW)
        .expect("reopen")
        .ticket;
    assert!(reopened.done_out_of_order.is_some());
}
