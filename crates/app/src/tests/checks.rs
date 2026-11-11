//! The loop the daemon closes: a hand-in is checked by the daemon (the project's check command
//! on the ship), the reviewer agent judges each acceptance item, and a card that does not pass
//! goes back to the worker, a round each, until the project's round limit. Only then the card
//! waits for the person.

use std::cell::RefCell;
use std::collections::VecDeque;

use toknila_core::agent::AgentEvent;
use toknila_core::{
    Actor, CommentKind, ItemOutcome, ItemVerdict, ReviewMode, ReviewTurn, Ticket, TicketStatus,
    Verdict,
};

use super::Mem;
use super::runs::{a, finish, settle, with_agents};
use crate::checks::{CheckError, CheckResult, CheckSpec, Checker};
use crate::external::ExternalEffect;
use crate::recovery::recover;
use crate::{SyncOutcome, agent, checks};
use toknila_core::migrations::Clash;

/// A checker that remembers what it was asked and answers from a queue (a pass when empty).
#[derive(Debug, Default)]
pub(super) struct FakeChecker {
    pub(super) asked: RefCell<Vec<CheckSpec>>,
    answers: RefCell<VecDeque<Result<CheckResult, CheckError>>>,
}

impl Checker for FakeChecker {
    fn run(&self, spec: &CheckSpec) -> Result<CheckResult, CheckError> {
        self.asked.borrow_mut().push(spec.clone());
        self.answers.borrow_mut().pop_front().unwrap_or_else(|| {
            Ok(CheckResult {
                success: true,
                code: Some(0),
                timed_out: false,
                output: "all green".to_owned(),
            })
        })
    }
}

fn fails(output: &str) -> Result<CheckResult, CheckError> {
    Ok(CheckResult {
        success: false,
        code: Some(101),
        timed_out: false,
        output: output.to_owned(),
    })
}

fn answer(mem: &Mem, answers: impl IntoIterator<Item = Result<CheckResult, CheckError>>) {
    mem.checker.answers.borrow_mut().extend(answers);
}

/// A project with a check command and process agents, card A running on its ship.
fn running(mode: ReviewMode, rounds: u32) -> Mem {
    let mut mem = with_agents();
    mem.project.check_command = Some("cargo xtask verify".to_owned());
    mem.project.default_review_mode = mode;
    mem.project.max_review_rounds = rounds;
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    assert_eq!(a(&mem).status, TicketStatus::Running);
    mem
}

fn verdict(mem: &mut Mem, verdict: Verdict, comment: &str, items: Vec<ItemVerdict>) {
    let t = a(mem);
    let event = AgentEvent::ReviewVerdict {
        verdict,
        comment: comment.to_owned(),
        items,
    };
    agent::ticket_event(mem, &t, event).expect("verdict");
}

fn instructions(mem: &Mem) -> Vec<(Actor, String)> {
    mem.comments
        .iter()
        .filter(|c| c.kind == CommentKind::Instruction)
        .map(|c| (c.author, c.body.gist.clone()))
        .collect()
}

fn who_and_verdict(t: &Ticket) -> Vec<(Actor, Verdict)> {
    t.review
        .rounds
        .iter()
        .map(|r| (r.reviewer, r.verdict))
        .collect()
}

#[test]
fn a_hand_in_runs_the_check_in_the_ship_before_anyone_reviews() {
    let mut mem = running(ReviewMode::Both, 3);
    finish(&mut mem, "done".to_owned());
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Review);
    assert_eq!(t.review.turn, Some(ReviewTurn::Check));
    assert!(
        mem.checker.asked.borrow().is_empty(),
        "the command does not run inside the unit of work"
    );
    assert!(
        !mem.deferred
            .iter()
            .any(|e| matches!(e, ExternalEffect::StartReviewer { .. })),
        "no reviewer before the check passes"
    );

    settle(&mut mem);
    let asked = mem.checker.asked.borrow().clone();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].command, "cargo xtask verify");
    assert_eq!(asked[0].dir, mem.ships[0].path, "run in the ship");
    let t = a(&mem);
    assert_eq!(
        t.review.turn,
        Some(ReviewTurn::Agent { decides: false }),
        "the pass hands the card to the reviewer"
    );
    assert_eq!(who_and_verdict(&t), [(Actor::System, Verdict::Approve)]);
}

#[test]
fn a_failing_check_returns_the_card_to_the_worker_with_the_output_and_no_person() {
    let mut mem = running(ReviewMode::Human, 3);
    answer(
        &mem,
        [fails("error[E0308]: mismatched types\n --> src/lib.rs:3")],
    );
    finish(&mut mem, "done".to_owned());
    settle(&mut mem);

    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Running, "back to the worker");
    assert_eq!(t.review.turn, None);
    assert_eq!(
        who_and_verdict(&t),
        [(Actor::System, Verdict::RequestChanges)]
    );
    let sent: Vec<_> = mem
        .comments
        .iter()
        .filter(|c| c.kind == CommentKind::Instruction)
        .collect();
    assert_eq!(sent.len(), 1, "{:?}", instructions(&mem));
    assert_eq!(sent[0].author, Actor::System);
    let text = format!(
        "{} {}",
        sent[0].body.gist,
        sent[0].body.what.clone().unwrap_or_default()
    );
    assert!(text.contains("round 1 of 3"), "{text}");
    assert!(text.contains("mismatched types"), "{text}");
    assert!(text.contains("cargo xtask verify"), "{text}");
}

#[test]
fn after_the_round_limit_the_card_waits_for_the_person_with_the_whole_history() {
    let mut mem = running(ReviewMode::Both, 2);
    answer(&mem, [fails("first"), fails("second"), fails("third")]);
    for round in 1..=2 {
        finish(&mut mem, format!("try {round}"));
        settle(&mut mem);
        assert_eq!(a(&mem).status, TicketStatus::Running, "round {round}");
    }
    finish(&mut mem, "try 3".to_owned());
    settle(&mut mem);
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Review);
    assert_eq!(t.review.turn, Some(ReviewTurn::Person));
    assert_eq!(
        who_and_verdict(&t),
        [(Actor::System, Verdict::RequestChanges); 3],
        "the whole history"
    );
    assert_eq!(
        instructions(&mem).len(),
        2,
        "the third failure is not sent back"
    );
    let reports: Vec<_> = mem
        .comments
        .iter()
        .filter(|c| c.body.gist.contains("2 rounds"))
        .collect();
    assert_eq!(reports.len(), 1);
}

#[test]
fn the_reviewers_request_changes_returns_the_card_and_counts_as_a_round() {
    let mut mem = running(ReviewMode::Both, 1);
    finish(&mut mem, "done".to_owned());
    settle(&mut mem);
    verdict(
        &mut mem,
        Verdict::RequestChanges,
        "handle the empty list",
        vec![],
    );
    settle(&mut mem);
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Running, "returned by the reviewer");
    assert_eq!(
        who_and_verdict(&t),
        [
            (Actor::System, Verdict::Approve),
            (Actor::Agent, Verdict::RequestChanges)
        ]
    );
    assert_eq!(t.review.auto_returns(), 1);

    // The limit is 1: the next request for changes waits for the person.
    finish(&mut mem, "again".to_owned());
    settle(&mut mem);
    verdict(&mut mem, Verdict::RequestChanges, "still not", vec![]);
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Review);
    assert_eq!(t.review.turn, Some(ReviewTurn::Person));
    assert_eq!(t.review.rounds.len(), 4);
}

#[test]
fn the_reviewers_verdict_names_each_acceptance_item() {
    let mut mem = running(ReviewMode::Both, 3);
    let t = a(&mem);
    let acceptance = vec!["first holds".to_owned(), "second holds".to_owned()];
    mem.view
        .tickets
        .iter_mut()
        .find(|x| x.id == t.id)
        .expect("ticket")
        .acceptance
        .clone_from(&acceptance);
    finish(&mut mem, "done".to_owned());
    settle(&mut mem);

    let t = a(&mem);
    let item = |n, outcome| ItemVerdict {
        item: n,
        outcome,
        basis: toknila_core::ItemBasis::Ran,
        note: "checked".to_owned(),
    };
    let t2 = t.clone();
    let event = AgentEvent::ReviewVerdict {
        verdict: Verdict::RequestChanges,
        comment: "fix 2".to_owned(),
        items: vec![item(1, ItemOutcome::Pass)],
    };
    assert!(
        agent::ticket_event(&mut mem, &t2, event).is_err(),
        "an item is missing"
    );

    let items = vec![item(1, ItemOutcome::Pass), item(2, ItemOutcome::Fail)];
    verdict(&mut mem, Verdict::RequestChanges, "fix 2", items);
    let round = a(&mem).review.rounds.last().cloned().expect("round");
    assert_eq!(
        round.comment,
        "PASS 1. first holds - checked [ran]\nFAIL 2. second holds - checked [ran]\nfix 2"
    );
}

#[test]
fn a_check_that_cannot_run_leaves_the_card_to_the_person() {
    let mut mem = running(ReviewMode::Both, 3);
    answer(&mem, [Err(CheckError::Io("no shell".to_owned()))]);
    finish(&mut mem, "done".to_owned());
    settle(&mut mem);
    let t = a(&mem);
    assert_eq!(
        (t.status, t.review.turn),
        (TicketStatus::Review, Some(ReviewTurn::Person))
    );
}

#[test]
fn a_check_whose_program_is_not_found_is_no_round_and_leaves_the_card_to_the_person() {
    let mut mem = running(ReviewMode::Both, 3);
    mem.project.check_command = Some("cargo test".to_owned());
    answer(
        &mem,
        [Ok(CheckResult {
            success: false,
            code: Some(127),
            timed_out: false,
            output: "sh: cargo: command not found".to_owned(),
        })],
    );
    finish(&mut mem, "done".to_owned());
    settle(&mut mem);
    let t = a(&mem);
    assert_eq!(
        (t.status, t.review.turn),
        (TicketStatus::Review, Some(ReviewTurn::Person))
    );
    assert!(t.review.rounds.is_empty(), "{:?}", t.review.rounds);
    assert_eq!(t.review.auto_returns(), 0);
    assert!(instructions(&mem).is_empty(), "{:?}", instructions(&mem));
}

#[test]
fn a_card_whose_check_the_restart_cut_short_is_queued_again_and_checked() {
    let mut mem = running(ReviewMode::Both, 3);
    finish(&mut mem, "done".to_owned());
    mem.deferred.clear(); // the daemon stopped before the host ran it
    let resumed = checks::resume(&mut mem).expect("resume");
    assert_eq!(resumed.len(), 1);
    assert_eq!(a(&mem).review.turn, Some(ReviewTurn::Check));
    assert_eq!(mem.deferred.len(), 1, "the check is queued for the host");
    settle(&mut mem);
    assert_eq!(
        a(&mem).review.turn,
        Some(ReviewTurn::Agent { decides: false })
    );
    assert_eq!(mem.checker.asked.borrow().len(), 1);
}

#[test]
fn a_project_without_a_check_command_still_syncs_and_the_turn_passes_on() {
    let mut mem = running(ReviewMode::Human, 3);
    mem.project.check_command = None;
    finish(&mut mem, "done".to_owned());
    assert_eq!(
        a(&mem).review.turn,
        Some(ReviewTurn::Check),
        "the sync runs"
    );
    settle(&mut mem);
    assert_eq!(mem.merger.synced.borrow().len(), 1, "the ship is synced");
    assert!(mem.checker.asked.borrow().is_empty(), "no command ran");
    assert_eq!(a(&mem).review.turn, Some(ReviewTurn::Person));
}

#[test]
fn a_hand_in_brings_the_ship_up_to_its_base_before_the_check_runs() {
    let mut mem = running(ReviewMode::Both, 3);
    finish(&mut mem, "done".to_owned());
    settle(&mut mem);
    let synced = mem.merger.synced.borrow().clone();
    assert_eq!(synced.len(), 1, "the ship is synced once");
    assert_eq!(synced[0].path, mem.ships[0].path, "in the ship's worktree");
    assert_eq!(mem.checker.asked.borrow().len(), 1, "then the check reruns");
    assert_eq!(
        a(&mem).review.turn,
        Some(ReviewTurn::Agent { decides: false })
    );
}

#[test]
fn a_conflict_with_the_base_returns_the_card_to_the_worker_naming_the_files_and_the_migration() {
    let mut mem = running(ReviewMode::Human, 3);
    *mem.merger.sync_answer.borrow_mut() = Ok(SyncOutcome::Conflict {
        files: vec!["src/lib.rs".to_owned()],
        migrations: vec![Clash {
            number: 16,
            ship: "mine".to_owned(),
            base: "theirs".to_owned(),
        }],
    });
    finish(&mut mem, "done".to_owned());
    settle(&mut mem);

    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Running, "back to the worker");
    assert_eq!(t.review.turn, None, "nobody is asked");
    assert!(
        mem.checker.asked.borrow().is_empty(),
        "the check does not run on a ship that did not merge"
    );
    let sent: Vec<_> = mem
        .comments
        .iter()
        .filter(|c| c.kind == CommentKind::Instruction)
        .collect();
    assert_eq!(sent.len(), 1, "{:?}", instructions(&mem));
    assert_eq!(sent[0].author, Actor::System);
    let text = format!(
        "{} {}",
        sent[0].body.gist,
        sent[0].body.what.clone().unwrap_or_default()
    );
    assert!(text.contains("src/lib.rs"), "{text}");
    assert!(text.contains("migration 16 is taken twice"), "{text}");
    assert!(text.contains("\"theirs\""), "{text}");
}

#[test]
fn a_sync_that_cannot_run_sends_the_card_to_the_person() {
    let mut mem = running(ReviewMode::Human, 3);
    *mem.merger.sync_answer.borrow_mut() = Err(crate::MergeError::Io("disk".to_owned()));
    finish(&mut mem, "done".to_owned());
    settle(&mut mem);
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Review);
    assert_eq!(t.review.turn, Some(ReviewTurn::Person));
}
