use super::*;
use crate::thread::{ASK_DESIGN_TEXT, require_design};
use crate::{Actor, Thread, ThreadId, ThreadKind, ThreadState, thread as threads};

fn names(t: &Ticket) -> Vec<&'static str> {
    TicketAction::allowed(t)
        .into_iter()
        .map(TicketAction::name)
        .collect()
}

#[test]
fn asking_for_a_design_on_a_card_that_has_not_started_sets_the_flag_and_nothing_else() {
    for from in [TicketStatus::Backlog, TicketStatus::Ready] {
        let t = ok(from, TicketEvent::AskDesign);
        assert!(t.ticket.design_first, "{from:?}");
        assert_eq!(t.ticket.status, from);
        assert!(t.effects.is_empty(), "{from:?}: {:?}", t.effects);
        assert_eq!(check_invariants(&t.ticket), Ok(()));
    }
}

#[test]
fn asking_for_a_design_on_a_running_card_also_sends_the_instruction() {
    for from in [TicketStatus::Running, TicketStatus::Waiting] {
        let t = ok(from, TicketEvent::AskDesign);
        assert!(t.ticket.design_first, "{from:?}");
        assert_eq!(t.ticket.status, from);
        assert_eq!(
            t.effects,
            [Effect::RecordInstruction {
                text: ASK_DESIGN_TEXT.to_owned()
            }]
        );
        assert!(ASK_DESIGN_TEXT.starts_with("Before you go further, open a design thread"));
    }
}

#[test]
fn a_card_in_review_or_done_cannot_ask_for_a_design_and_a_card_that_did_cannot_again() {
    for from in [
        TicketStatus::Review,
        TicketStatus::Merged,
        TicketStatus::Released,
        TicketStatus::Cancelled,
    ] {
        let t = ticket_in(from);
        assert!(apply(&t, &TicketEvent::AskDesign, NOW).is_err(), "{from:?}");
        assert!(!names(&t).contains(&"ask_design"), "{from:?}");
    }
    let mut asked = ticket_in(TicketStatus::Ready);
    assert!(names(&asked).contains(&"ask_design"));
    asked.design_first = true;
    assert!(apply(&asked, &TicketEvent::AskDesign, NOW).is_err());
    assert!(!names(&asked).contains(&"ask_design"));
}

fn thread_of(t: &Ticket, state: ThreadState, approved_version: Option<u32>) -> Thread {
    let mut th = threads::open(
        ThreadId::new(),
        t.id,
        1,
        ThreadKind::Design,
        "Screen",
        Actor::Agent,
        NOW,
    )
    .expect("open");
    th.state = state;
    th.approved_version = approved_version;
    th
}

#[test]
fn a_design_first_card_cannot_hand_in_before_a_design_version_is_approved() {
    let mut t = ticket_in(TicketStatus::Running);
    assert_eq!(require_design(&t, &[]), Ok(()), "not asked for: no gate");
    t.design_first = true;
    let err = require_design(&t, &[]).expect_err("no thread");
    assert!(err.to_string().contains("open_thread"), "{err}");
    assert!(
        err.to_string().contains("wait for the person to approve"),
        "{err}"
    );
    let open = thread_of(&t, ThreadState::Open, None);
    assert!(require_design(&t, &[open]).is_err());
    let no_version = thread_of(&t, ThreadState::Approved, None);
    assert!(
        require_design(&t, &[no_version]).is_err(),
        "approved without a version"
    );
    let approved = thread_of(&t, ThreadState::Approved, Some(2));
    assert_eq!(require_design(&t, &[approved]), Ok(()));
}
