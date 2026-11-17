//! A project's merge line through the use cases: accepted cards merge one at a time in the order
//! they were accepted, each holding its turn across sync, re-check and merge; a release run holds
//! the same line until its Tag; a turn is freed when its check fails and when the daemon starts
//! again. With a fake merger whose merges move main, a fake checker and release runs kept the way
//! the host keeps them.

use toknila_core::agent::AgentEvent;
use toknila_core::machine::TicketEvent;
use toknila_core::merge_line::ReleaseTurn;
use toknila_core::{ReleaseId, ReviewTurn, TicketStatus, Timestamp};

use super::Mem;
use super::checks::{answer, fails};
use super::runs::{settle, with_agents};
use crate::external::{self, ExternalEffect, Host};
use crate::recovery::recover;
use crate::{Seen, agent, merges, tickets};

fn main_at(mem: &Mem, base: &str) {
    *mem.merger.seen_answer.borrow_mut() = Some(Seen {
        base: base.to_owned(),
        head: format!("head-at-{base}"),
        commits: Some(1),
    });
}

fn hand_in(mem: &mut Mem, title: &str) {
    let t = mem.ticket_by_title(title);
    let summary = toknila_core::handin::from_plain("done");
    agent::ticket_event(mem, &t, AgentEvent::Finished { summary }).expect("finished");
    settle(mem);
    assert_eq!(
        mem.ticket_by_title(title).review.turn,
        Some(ReviewTurn::Person)
    );
}

/// Project DEMO with a check command and process agents; A and B do not wait for each other.
/// B's hand-in was checked with main at `m1`, A's after main moved to `m2`; both wait for a
/// person. Merges move main.
fn two_checked() -> Mem {
    let mut mem = with_agents();
    mem.project.check_command = Some("cargo xtask verify".to_owned());
    mem.view.edges.clear();
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    main_at(&mem, "m1");
    hand_in(&mut mem, "B");
    main_at(&mem, "m2");
    hand_in(&mut mem, "A");
    mem.merger.moves_main.set(true);
    mem.merger.synced.borrow_mut().clear();
    mem.checker.asked.borrow_mut().clear();
    mem
}

fn accept(mem: &mut Mem, title: &str, at: i64) {
    mem.now = Timestamp(at);
    let t = mem.ticket_by_title(title);
    tickets::act(mem, &t, TicketEvent::Accept { comment: None }).expect("accept");
}

/// The keys of the cards whose merge is queued for the host.
fn queued_merges(mem: &Mem) -> Vec<String> {
    mem.deferred
        .iter()
        .filter_map(|e| match e {
            ExternalEffect::Merge { ticket, .. } => Some(mem.ticket_by_id(*ticket).key.to_string()),
            _ => None,
        })
        .collect()
}

/// The end of a card's branch and ship path: `/demo-1-a`.
fn slug(t: &toknila_core::Ticket) -> String {
    format!(
        "/{}-{}",
        t.key.to_string().to_lowercase(),
        t.title.to_lowercase()
    )
}

/// The keys of the cards the host merged, in order.
fn merged_in_order(mem: &Mem) -> Vec<String> {
    let asked = mem.merger.asked.borrow();
    asked
        .iter()
        .filter_map(|s| {
            let t = mem
                .view
                .tickets
                .iter()
                .find(|t| s.branch.ends_with(&slug(t)));
            t.map(|t| t.key.to_string())
        })
        .collect()
}

/// How many checks ran on the card's ship.
fn checks_of(mem: &Mem, title: &str) -> usize {
    let end = slug(&mem.ticket_by_title(title));
    let asked = mem.checker.asked.borrow();
    asked.iter().filter(|c| c.dir.ends_with(&end)).count()
}

/// What the card says while it waits in the line.
fn wait_words(mem: &Mem, title: &str) -> Option<String> {
    let t = mem.ticket_by_title(title);
    let line = merges::line_of(mem, mem.project.id).expect("line");
    line.card_wait(t.id).map(|w| w.card_words())
}

/// Runs one batch of what was queued for the host and records it.
fn step(mem: &mut Mem) {
    let queued = std::mem::take(&mut mem.deferred);
    let host = Host {
        yard: &mem.yard,
        merger: &mem.merger,
        runner: &mem.runner,
        checker: &mem.checker,
        gate: &crate::OpenGate,
        progress: &crate::NoProgress,
    };
    let outcomes = external::execute(host, queued);
    external::apply(mem, outcomes).expect("apply outcomes");
}

/// A release run of 0.3.8 begins: it joins the line, not holding the turn yet.
fn release_begins(mem: &mut Mem) -> ReleaseId {
    let release = ReleaseId::new();
    mem.release_turns.push(ReleaseTurn {
        release,
        version: "0.3.8".to_owned(),
        holds: false,
    });
    release
}

/// The release asks for its turn: `None` when it holds it, else what it waits for.
fn release_asks(mem: &mut Mem, release: ReleaseId) -> Option<String> {
    let project = mem.project.id;
    merges::take_release_turn(mem, project, release)
        .expect("ask")
        .map(|w| w.words())
}

/// The release's Tag passed: its run leaves the line, and the host gives the turn on.
fn release_tagged(mem: &mut Mem) {
    mem.release_turns.clear();
    let project = mem.project.id;
    merges::advance(mem, project).expect("advance");
}

fn status(mem: &Mem, title: &str) -> TicketStatus {
    mem.ticket_by_title(title).status
}

#[test]
fn two_accepted_cards_merge_in_accept_order_and_the_first_rechecks_once() {
    let mut mem = two_checked();
    // B is accepted first and needs a re-check (main moved since its check); A is ready.
    accept(&mut mem, "B", 2_000);
    accept(&mut mem, "A", 3_000);
    assert_eq!(
        queued_merges(&mem),
        ["DEMO-2"],
        "A is ready, but B holds the turn: only B's merge is queued"
    );
    assert_eq!(
        wait_words(&mem, "A").as_deref(),
        Some("accepted; waits for DEMO-2 to merge (#2 in line)")
    );
    settle(&mut mem);
    assert_eq!(
        (status(&mem, "A"), status(&mem, "B")),
        (TicketStatus::Merged, TicketStatus::Merged)
    );
    assert_eq!(
        merged_in_order(&mem),
        ["DEMO-2", "DEMO-1"],
        "in Accept order"
    );
    assert_eq!(
        checks_of(&mem, "B"),
        1,
        "B's re-check ran once: main stayed"
    );
    assert_eq!(
        checks_of(&mem, "A"),
        1,
        "A was checked again once, against main with B in it"
    );
    assert_eq!(wait_words(&mem, "A"), None);
}

#[test]
fn a_card_accepted_during_a_release_merges_after_the_release_tag() {
    let mut mem = two_checked();
    let release = release_begins(&mut mem);
    assert_eq!(release_asks(&mut mem, release), None, "the line was free");
    accept(&mut mem, "A", 2_000);
    assert!(
        queued_merges(&mem).is_empty(),
        "no card merges while a release runs"
    );
    assert_eq!(
        wait_words(&mem, "A").as_deref(),
        Some("accepted; waits for the release of 0.3.8")
    );
    settle(&mut mem);
    assert!(
        mem.merger.asked.borrow().is_empty() && mem.merger.synced.borrow().is_empty(),
        "main stays where the release began: its Tag finds no moved main"
    );
    assert_eq!(status(&mem, "A"), TicketStatus::Review);
    release_tagged(&mut mem);
    assert_eq!(
        queued_merges(&mem),
        ["DEMO-1"],
        "after the Tag the card's turn comes"
    );
    settle(&mut mem);
    assert_eq!(status(&mem, "A"), TicketStatus::Merged);
}

#[test]
fn a_release_started_while_a_cards_turn_runs_waits_for_it() {
    let mut mem = two_checked();
    accept(&mut mem, "B", 2_000);
    step(&mut mem);
    let b = mem.ticket_by_title("B");
    assert_eq!(b.review.turn, Some(ReviewTurn::Check), "B is checked again");
    let release = release_begins(&mut mem);
    assert_eq!(
        release_asks(&mut mem, release).as_deref(),
        Some("waits for DEMO-2 to merge (#2 in line)")
    );
    accept(&mut mem, "A", 3_000);
    assert_eq!(
        wait_words(&mem, "A").as_deref(),
        Some("accepted; waits for the release of 0.3.8"),
        "the release goes ahead of the cards that wait"
    );
    settle(&mut mem);
    assert_eq!(status(&mem, "B"), TicketStatus::Merged);
    assert_eq!(
        status(&mem, "A"),
        TicketStatus::Review,
        "A waits for the release"
    );
    assert_eq!(
        release_asks(&mut mem, release),
        None,
        "B merged: the release's turn"
    );
    assert!(mem.release_turns[0].holds);
    release_tagged(&mut mem);
    settle(&mut mem);
    assert_eq!(status(&mem, "A"), TicketStatus::Merged);
    assert_eq!(merged_in_order(&mem), ["DEMO-2", "DEMO-1"]);
}

#[test]
fn a_turn_is_freed_when_its_recheck_fails() {
    let mut mem = two_checked();
    accept(&mut mem, "B", 2_000);
    accept(&mut mem, "A", 3_000);
    assert_eq!(queued_merges(&mem), ["DEMO-2"]);
    // A failure runs once more in the same turn of the check line: both runs fail.
    let failed = || fails("test result: FAILED. 1 failed");
    answer(&mem, [failed(), failed()]);
    settle(&mut mem);
    let b = mem.ticket_by_title("B");
    assert_eq!(b.status, TicketStatus::Running, "back to the worker");
    assert_eq!(b.review.merge_line, None, "out of the line, its turn freed");
    assert_eq!(
        status(&mem, "A"),
        TicketStatus::Merged,
        "the next card merged"
    );
}

#[test]
fn a_restart_frees_the_release_turn_and_queues_the_accepted_cards_in_their_order() {
    let mut mem = two_checked();
    let release = release_begins(&mut mem);
    assert_eq!(release_asks(&mut mem, release), None);
    accept(&mut mem, "A", 2_000);
    accept(&mut mem, "B", 3_000);
    settle(&mut mem);
    assert!(mem.merger.asked.borrow().is_empty());
    // The daemon stops: the release run is interrupted and nobody holds its turn. A was accepted
    // by a daemon that kept no line: it has no place yet.
    mem.release_turns.clear();
    mem.deferred.clear();
    let a = mem.ticket_by_title("A");
    with_chain_edit(&mut mem, a.id, |t| t.review.merge_line = None);
    recover(&mut mem).expect("recover");
    assert_eq!(
        queued_merges(&mem),
        ["DEMO-1"],
        "the first accepted takes the turn"
    );
    assert_eq!(
        wait_words(&mem, "B").as_deref(),
        Some("accepted; waits for DEMO-1 to merge (#2 in line)")
    );
    settle(&mut mem);
    assert_eq!(merged_in_order(&mem), ["DEMO-1", "DEMO-2"]);
}

/// Changes a card as a store would have it, without the scheduler or the line acting on it.
fn with_chain_edit(
    mem: &mut Mem,
    id: toknila_core::TicketId,
    f: impl FnOnce(&mut toknila_core::Ticket),
) {
    let chain = mem.ticket_by_id(id).chain_id;
    crate::chain::run_chain(mem, chain, |ctx| {
        ctx.edit_ticket(id, f).map_err(crate::AppError::from)
    })
    .expect("edit");
}
