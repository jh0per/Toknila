//! The parallel agent limit through the use cases, with simulated agents: a start beyond the
//! limit queues, a freed slot starts the next card, and a changed limit acts at once.

use toknila_core::machine::TicketEvent;
use toknila_core::{Actor, Ticket, TicketDraft, TicketId, TicketStatus};

use super::Mem;
use crate::chains::{self, ChainSettings};
use crate::projects::{self, PatchProject};
use crate::tickets::TicketAction;
use crate::{ErrorKind, schedule, tickets, with_chain};

/// Project DEMO (2 slots), chain `demo` without autostart, `n` independent cards, all Ready.
fn world(n: usize) -> Mem {
    let mut mem = Mem::demo();
    mem.view.chain.autostart = false;
    mem.view.edges.clear();
    mem.view.tickets.clear();
    for i in 1..=n {
        let key = format!("DEMO-{i}").parse().expect("key");
        let draft = TicketDraft {
            title: format!("card {i}"),
            ..TicketDraft::default()
        };
        let t = Ticket::new_backlog(TicketId::new(), mem.view.chain.id, key, draft, mem.now);
        mem.view.tickets.push(t);
    }
    let chain = mem.view.chain.id;
    with_chain(&mut mem, chain, |_| Ok::<_, crate::AppError>(())).expect("settle");
    mem
}

fn card(mem: &Mem, n: usize) -> Ticket {
    mem.ticket_by_title(&format!("DEMO-{n}"))
}

fn status(mem: &Mem, n: usize) -> TicketStatus {
    card(mem, n).status
}

fn act(mem: &mut Mem, n: usize, event: TicketEvent) {
    let t = card(mem, n);
    tickets::act(mem, &t, event).expect("act");
}

fn start(mem: &mut Mem, n: usize) {
    let event = TicketAction::Start
        .event(None, Actor::Human)
        .expect("event");
    act(mem, n, event);
}

fn finish(mem: &mut Mem, n: usize) {
    let event = TicketEvent::Finished {
        summary: "done".to_owned(),
        open_threads: Vec::new(),
        mode: toknila_core::ReviewMode::Human,
        check: false,
        max_rounds: 3,
    };
    act(mem, n, event);
}

fn position(mem: &Mem, n: usize) -> Option<u32> {
    let id = card(mem, n).id;
    schedule::queue_positions(mem)
        .expect("positions")
        .get(&id)
        .copied()
}

fn started_three() -> Mem {
    let mut mem = world(3);
    for n in 1..=3 {
        start(&mut mem, n);
    }
    mem
}

#[test]
fn starting_three_cards_with_two_slots_runs_two_and_queues_one() {
    let mem = started_three();
    assert_eq!(status(&mem, 1), TicketStatus::Running);
    assert_eq!(status(&mem, 2), TicketStatus::Running);
    assert_eq!(status(&mem, 3), TicketStatus::Ready);
    assert!(card(&mem, 3).queued);
    assert_eq!(position(&mem, 3), Some(1));
    assert_eq!(position(&mem, 1), None);
}

#[test]
fn finishing_a_card_starts_the_queued_one() {
    let mut mem = started_three();
    finish(&mut mem, 1);
    assert_eq!(status(&mem, 1), TicketStatus::Review);
    assert_eq!(status(&mem, 3), TicketStatus::Running);
    assert!(!card(&mem, 3).queued);
    assert_eq!(position(&mem, 3), None);
}

#[test]
fn a_cancelled_card_frees_its_slot_and_the_queue_moves_up() {
    let mut mem = world(4);
    for n in 1..=4 {
        start(&mut mem, n);
    }
    assert_eq!(position(&mem, 3), Some(1));
    assert_eq!(position(&mem, 4), Some(2));
    act(
        &mut mem,
        1,
        TicketEvent::Cancel {
            reason: "no".to_owned(),
        },
    );
    assert_eq!(status(&mem, 3), TicketStatus::Running);
    assert_eq!(position(&mem, 4), Some(1));
}

#[test]
fn a_card_nobody_started_is_never_started_by_the_queue() {
    let mut mem = world(3);
    start(&mut mem, 1);
    start(&mut mem, 2);
    finish(&mut mem, 1);
    assert_eq!(status(&mem, 3), TicketStatus::Ready);
    assert!(!card(&mem, 3).queued);
    assert_eq!(position(&mem, 3), None);
}

#[test]
fn a_higher_limit_starts_queued_cards_at_once() {
    let mut mem = started_three();
    let patch = PatchProject {
        max_parallel: Some(3),
        ..PatchProject::default()
    };
    let p = projects::patch(&mut mem, "demo", patch).expect("patch");
    assert_eq!(p.max_parallel, 3);
    assert_eq!(status(&mem, 3), TicketStatus::Running);
}

#[test]
fn a_lower_limit_never_stops_a_running_agent() {
    let mut mem = started_three();
    let patch = PatchProject {
        max_parallel: Some(1),
        ..PatchProject::default()
    };
    projects::patch(&mut mem, "demo", patch).expect("patch");
    assert_eq!(status(&mem, 1), TicketStatus::Running);
    assert_eq!(status(&mem, 2), TicketStatus::Running);
    assert_eq!(status(&mem, 3), TicketStatus::Ready);
    assert!(
        !mem.deferred
            .iter()
            .any(|e| matches!(e, crate::external::ExternalEffect::StopRunner { .. }))
    );
    // One agent is still working after the first finishes, so the queue holds.
    finish(&mut mem, 1);
    assert_eq!(status(&mem, 3), TicketStatus::Ready);
    finish(&mut mem, 2);
    assert_eq!(status(&mem, 3), TicketStatus::Running);
}

#[test]
fn a_limit_outside_the_range_is_refused() {
    let mut mem = world(1);
    for bad in [0, 65] {
        let patch = PatchProject {
            max_parallel: Some(bad),
            ..PatchProject::default()
        };
        let err = projects::patch(&mut mem, "DEMO", patch).expect_err("refused");
        assert_eq!(err.kind, ErrorKind::BadRequest);
    }
    assert_eq!(mem.project.max_parallel, 2);
}

#[test]
fn starting_a_card_twice_while_it_waits_is_a_conflict() {
    let mut mem = started_three();
    let t = card(&mem, 3);
    let event = TicketAction::Start
        .event(None, Actor::Human)
        .expect("event");
    let err = tickets::act(&mut mem, &t, event).expect_err("already queued");
    assert_eq!(err.kind, ErrorKind::Conflict);
}

#[test]
fn turning_autostart_on_starts_the_ready_cards_and_is_journaled_and_off_starts_none() {
    let mut mem = world(2);
    assert_eq!(status(&mem, 1), TicketStatus::Ready, "control: waits");
    mem.events.clear();
    let on = ChainSettings {
        autostart: Some(true),
        ..ChainSettings::default()
    };
    chains::set_settings(&mut mem, "demo", None, on).expect("on");
    assert!(mem.view.chain.autostart);
    assert_eq!(status(&mem, 1), TicketStatus::Running);
    assert_eq!(status(&mem, 2), TicketStatus::Running);
    assert!(
        mem.kinds().contains(&"chain.updated".to_owned()),
        "{:?}",
        mem.kinds()
    );

    let off = ChainSettings {
        autostart: Some(false),
        ..ChainSettings::default()
    };
    chains::set_settings(&mut mem, "demo", None, off).expect("off");
    assert!(!mem.view.chain.autostart);
    // A field left out is kept.
    chains::set_settings(&mut mem, "demo", None, ChainSettings::default()).expect("noop");
    assert!(!mem.view.chain.autostart);
}

#[test]
fn turning_autostart_on_respects_the_slot_limit() {
    let mut mem = world(3);
    let on = ChainSettings {
        autostart: Some(true),
        ..ChainSettings::default()
    };
    chains::set_settings(&mut mem, "demo", None, on).expect("on");
    let running = (1..=3)
        .filter(|n| status(&mem, *n) == TicketStatus::Running)
        .count();
    assert_eq!(running, 2, "two slots; the third waits in the queue");
}
