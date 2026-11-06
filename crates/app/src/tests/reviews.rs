//! Review through the use cases: the three modes with the simulated reviewer (no process, a
//! verdict sent as an event), and a process reviewer that runs next to the worker.

use toknila_core::agent::AgentEvent;
use toknila_core::machine::TicketEvent;
use toknila_core::{
    Actor, AgentRole, CommentKind, ProcessExit, ReviewMode, ReviewTurn, Ticket, TicketStatus,
    Verdict,
};

use super::Mem;
use super::runs::{FakeRunner, a, act, finish, run_of, settle, with_agents};
use crate::external::ExternalEffect;
use crate::recovery::recover;
use crate::runner::{RunEnded, RunRequest};
use crate::{agent, reviews, runs, tickets};

/// A simulated worker (profile `sim`): it starts no process, so a test plays it.
fn sim_in_review(mode: ReviewMode) -> Mem {
    let mut mem = Mem::demo();
    mem.project.default_review_mode = mode;
    recover(&mut mem).expect("settle");
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Running);
    finish(&mut mem, "done".to_owned());
    mem
}

fn verdict(mem: &mut Mem, verdict: Verdict, comment: &str) -> crate::AppResult<()> {
    let t = a(mem);
    let event = AgentEvent::ReviewVerdict {
        verdict,
        comment: comment.to_owned(),
    };
    agent::ticket_event(mem, &t, event).map(|_| ())
}

fn rounds(t: &Ticket) -> Vec<(u32, Actor, Verdict, String)> {
    t.review
        .rounds
        .iter()
        .map(|r| (r.number, r.reviewer, r.verdict, r.comment.clone()))
        .collect()
}

#[test]
fn human_mode_waits_for_a_person_who_accepts_with_remarks() {
    let mut mem = sim_in_review(ReviewMode::Human);
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Review);
    assert_eq!(t.review.turn, Some(ReviewTurn::Person));
    assert!(mem.deferred.is_empty(), "no reviewer in human mode");
    // The agent's verdict is not wanted when nobody asked for it.
    assert!(verdict(&mut mem, Verdict::Approve, "").is_err());

    let event = TicketEvent::Accept {
        comment: Some("good".to_owned()),
    };
    act(&mut mem, event);
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Merged);
    assert_eq!(
        rounds(&t),
        [(1, Actor::Human, Verdict::Approve, "good".to_owned())]
    );
}

#[test]
fn agent_mode_the_reviewers_approval_merges_and_its_remarks_return_the_card() {
    let mut mem = sim_in_review(ReviewMode::Agent);
    let t = a(&mem);
    assert_eq!(t.review.turn, Some(ReviewTurn::Agent { decides: true }));

    verdict(&mut mem, Verdict::RequestChanges, "handle the empty list").expect("verdict");
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Running, "back to the worker");
    let remarks: Vec<_> = mem
        .comments
        .iter()
        .filter(|c| c.kind == CommentKind::Instruction)
        .collect();
    assert_eq!(remarks.len(), 1);
    assert_eq!(remarks[0].author, Actor::Agent);
    assert_eq!(remarks[0].body.gist, "handle the empty list");
    assert!(
        remarks[0].delivered_at.is_some(),
        "a simulated worker reads the remarks at once"
    );

    finish(&mut mem, "fixed".to_owned());
    verdict(&mut mem, Verdict::Approve, "now fine").expect("verdict");
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Merged);
    assert_eq!(
        rounds(&t),
        [
            (
                1,
                Actor::Agent,
                Verdict::RequestChanges,
                "handle the empty list".to_owned()
            ),
            (2, Actor::Agent, Verdict::Approve, "now fine".to_owned()),
        ]
    );
}

#[test]
fn both_mode_the_agent_speaks_first_and_a_person_decides_with_its_remarks() {
    let mut mem = sim_in_review(ReviewMode::Both);
    assert_eq!(
        a(&mem).review.turn,
        Some(ReviewTurn::Agent { decides: false })
    );

    verdict(&mut mem, Verdict::RequestChanges, "rename the helper").expect("verdict");
    let t = a(&mem);
    assert_eq!(
        t.status,
        TicketStatus::Review,
        "the card waits for a person"
    );
    assert_eq!(t.review.turn, Some(ReviewTurn::Person));

    let comment = "agreed, and fix the look".to_owned();
    act(&mut mem, TicketEvent::Return { comment });
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Running);
    assert_eq!(
        rounds(&t)
            .iter()
            .map(|(n, who, v, _)| (*n, *who, *v))
            .collect::<Vec<_>>(),
        [
            (1, Actor::Agent, Verdict::RequestChanges),
            (2, Actor::Human, Verdict::RequestChanges)
        ]
    );
}

#[test]
fn the_mode_and_the_reviewer_resolve_ticket_then_chain_then_project() {
    let mut mem = Mem::demo();
    mem.project.default_review_mode = ReviewMode::Both;
    mem.project.default_reviewer = Some("p-reviewer".to_owned());
    let t = a(&mem);
    let view = reviews::view(&mem, &t).expect("view");
    assert_eq!(
        (view.mode, view.reviewer.as_str()),
        (ReviewMode::Both, "p-reviewer")
    );

    mem.view.chain.review_mode = Some(ReviewMode::Agent);
    mem.view.chain.reviewer = Some("c-reviewer".to_owned());
    let view = reviews::view(&mem, &t).expect("view");
    assert_eq!(
        (view.mode, view.reviewer.as_str()),
        (ReviewMode::Agent, "c-reviewer")
    );

    mem.view.tickets[0].review_mode = Some(ReviewMode::Human);
    mem.view.tickets[0].reviewer = Some("t-reviewer".to_owned());
    let t = a(&mem);
    let view = reviews::view(&mem, &t).expect("view");
    assert_eq!(
        (view.mode, view.reviewer.as_str()),
        (ReviewMode::Human, "t-reviewer")
    );
}

#[test]
fn settings_on_a_chain_and_a_project_are_kept_and_journaled() {
    let mut mem = Mem::demo();
    let settings = reviews::ReviewSettings {
        review_mode: Some(ReviewMode::Both),
        reviewer: Some(" qa ".to_owned()),
    };
    reviews::set_chain(&mut mem, "demo", settings.clone()).expect("chain");
    assert_eq!(mem.view.chain.review_mode, Some(ReviewMode::Both));
    assert_eq!(mem.view.chain.reviewer.as_deref(), Some("qa"));
    let p = reviews::set_project(&mut mem, "DEMO", settings).expect("project");
    assert_eq!(p.default_review_mode, ReviewMode::Both);
    assert_eq!(p.default_reviewer.as_deref(), Some("qa"));
    let clear = reviews::ReviewSettings {
        review_mode: None,
        reviewer: Some(String::new()),
    };
    reviews::set_project(&mut mem, "DEMO", clear).expect("project");
    assert_eq!(mem.project.default_reviewer, None);
    assert_eq!(mem.project.default_review_mode, ReviewMode::Both, "kept");
    let kinds = mem.kinds();
    assert!(kinds.contains(&"chain.updated".to_owned()), "{kinds:?}");
    assert!(kinds.contains(&"project.updated".to_owned()), "{kinds:?}");
}

// ---- a reviewer that is a process, next to a worker that is one ----

fn live(runner: &FakeRunner) -> Vec<AgentRole> {
    let mut roles: Vec<_> = runner.live.borrow().iter().map(|(_, r)| *r).collect();
    roles.sort_by_key(|r| *r as u8);
    roles
}

/// Worker and reviewer are processes (`fake` and `fake-reviewer`); the card is in Review.
fn process_review(mode: ReviewMode) -> Mem {
    let mut mem = with_agents();
    mem.project.default_review_mode = mode;
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    finish(&mut mem, "done".to_owned());
    settle(&mut mem);
    mem
}

fn reviewer_requests(mem: &Mem) -> Vec<RunRequest> {
    mem.runner
        .started
        .borrow()
        .iter()
        .filter(|r| r.role == AgentRole::Reviewer)
        .cloned()
        .collect()
}

fn ended(mem: &Mem, role: AgentRole, code: i32, stopped: bool) -> RunEnded {
    let t = a(mem);
    RunEnded {
        run: run_of(&RunRequest {
            ticket: t.id,
            key: t.key,
            role,
            profile: "fake".to_owned(),
            dir: "/ships/demo/demo/demo-1-a".to_owned(),
        }),
        exit: ProcessExit {
            code: Some(code),
            signal: None,
        },
        stopped,
    }
}

#[test]
fn the_reviewer_runs_next_to_the_worker_on_the_same_ship() {
    let mem = process_review(ReviewMode::Agent);
    let [req] = reviewer_requests(&mem).try_into().expect("one reviewer");
    assert_eq!(
        req.profile, "fake-reviewer",
        "named after the worker's profile"
    );
    assert_eq!(req.dir, "/ships/demo/demo/demo-1-a", "the worker's ship");
    assert_eq!(
        live(&mem.runner),
        [AgentRole::Worker, AgentRole::Reviewer],
        "the worker's runner stays alive during review"
    );
    assert_eq!(
        mem.runner
            .started
            .borrow()
            .iter()
            .filter(|r| r.role == AgentRole::Worker)
            .count(),
        1,
        "the worker was started once"
    );
}

#[test]
fn a_review_profile_setting_chooses_the_reviewer() {
    let mut mem = with_agents();
    mem.project.default_review_mode = ReviewMode::Both;
    mem.view.chain.reviewer = Some("strict".to_owned());
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    finish(&mut mem, "done".to_owned());
    settle(&mut mem);
    let [req] = reviewer_requests(&mem).try_into().expect("one reviewer");
    assert_eq!(req.profile, "strict");
}

#[test]
fn human_mode_starts_no_reviewer() {
    let mem = process_review(ReviewMode::Human);
    assert!(reviewer_requests(&mem).is_empty());
    assert_eq!(live(&mem.runner), [AgentRole::Worker]);
}

#[test]
fn a_return_goes_to_the_live_worker_session_and_ends_the_reviewer_only() {
    let mut mem = process_review(ReviewMode::Agent);
    let id = a(&mem).id;
    verdict(&mut mem, Verdict::RequestChanges, "fix the look").expect("verdict");
    settle(&mut mem);
    assert_eq!(a(&mem).status, TicketStatus::Running);
    assert_eq!(
        mem.runner
            .started
            .borrow()
            .iter()
            .filter(|r| r.role == AgentRole::Worker)
            .count(),
        1,
        "the live worker is kept, not started again"
    );
    assert!(live(&mem.runner).contains(&AgentRole::Worker));
    assert!(
        mem.runner.stopped.borrow().is_empty(),
        "nothing stopped whole"
    );
    let _ = id;
}

#[test]
fn a_return_starts_a_new_worker_run_when_the_worker_has_exited() {
    let mut mem = process_review(ReviewMode::Human);
    // The worker's process ended while the card waited in Review: not a crash.
    mem.runner.live.borrow_mut().clear();
    let e = ended(&mem, AgentRole::Worker, 0, false);
    runs::ended(&mut mem, e).expect("ended");
    assert_eq!(a(&mem).status, TicketStatus::Review, "the card stays put");

    let comment = "one more thing".to_owned();
    act(&mut mem, TicketEvent::Return { comment });
    settle(&mut mem);
    assert_eq!(a(&mem).status, TicketStatus::Running);
    let workers = mem
        .runner
        .started
        .borrow()
        .iter()
        .filter(|r| r.role == AgentRole::Worker)
        .count();
    assert_eq!(workers, 2, "a second worker run on the same ship");
}

#[test]
fn a_person_deciding_while_the_reviewer_runs_stops_it_and_not_the_worker() {
    let mut mem = process_review(ReviewMode::Both);
    let t = a(&mem);
    let comment = "no".to_owned();
    tickets::act(&mut mem, &t, TicketEvent::Return { comment }).expect("return");
    settle(&mut mem);
    assert_eq!(
        mem.runner.stopped_roles.borrow().as_slice(),
        [(t.id, AgentRole::Reviewer)]
    );
    assert!(mem.runner.stopped.borrow().is_empty());
    assert!(live(&mem.runner).contains(&AgentRole::Worker));
}

#[test]
fn accepting_stops_every_runner_of_the_ticket() {
    let mut mem = process_review(ReviewMode::Human);
    act(&mut mem, TicketEvent::Accept { comment: None });
    settle(&mut mem);
    assert_eq!(a(&mem).status, TicketStatus::Merged);
    let id = a(&mem).id;
    assert!(
        mem.runner.live.borrow().iter().all(|(t, _)| *t != id),
        "no runner of the accepted ticket is left"
    );
}

#[test]
fn a_reviewer_that_exits_without_a_verdict_hands_the_card_to_a_person() {
    let mut mem = process_review(ReviewMode::Agent);
    let e = ended(&mem, AgentRole::Reviewer, 0, false);
    runs::ended(&mut mem, e).expect("ended");
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Review);
    assert_eq!(t.review.turn, Some(ReviewTurn::Person));
    assert!(t.review.rounds.is_empty());
    let report = mem
        .comments
        .iter()
        .find(|c| c.kind == CommentKind::Report && c.body.gist.contains("exited without a verdict"))
        .expect("the card says why");
    assert_eq!(report.author, Actor::Agent);
}

#[test]
fn a_reviewer_that_exits_after_its_verdict_changes_nothing() {
    let mut mem = process_review(ReviewMode::Both);
    verdict(&mut mem, Verdict::Approve, "fine").expect("verdict");
    let before = a(&mem);
    let e = ended(&mem, AgentRole::Reviewer, 0, false);
    runs::ended(&mut mem, e).expect("ended");
    assert_eq!(a(&mem), before);
}

#[test]
fn a_reviewer_that_cannot_start_hands_the_card_to_a_person() {
    let mut mem = with_agents();
    mem.project.default_review_mode = ReviewMode::Agent;
    mem.view.chain.reviewer = Some("missing".to_owned());
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    finish(&mut mem, "done".to_owned());
    // Only the reviewer is refused; the worker already runs.
    *mem.runner.refuse.borrow_mut() =
        Some(crate::RunnerError::UnknownProfile("missing".to_owned()));
    settle(&mut mem);
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Review);
    assert_eq!(t.review.turn, Some(ReviewTurn::Person));
}

#[test]
fn a_restart_hands_a_card_whose_reviewer_was_lost_to_a_person() {
    let mut mem = process_review(ReviewMode::Agent);
    let live_reviewers = std::collections::HashSet::new();
    let key = a(&mem).key;
    assert_eq!(
        reviews::mark_lost(&mut mem, &live_reviewers).expect("lost"),
        [key]
    );
    assert_eq!(a(&mem).review.turn, Some(ReviewTurn::Person));
    assert!(
        reviews::mark_lost(&mut mem, &live_reviewers)
            .expect("lost")
            .is_empty()
    );
}

#[test]
fn a_simulated_reviewer_is_not_lost_by_a_restart() {
    let mut mem = sim_in_review(ReviewMode::Agent);
    let none = std::collections::HashSet::new();
    assert!(
        reviews::mark_lost(&mut mem, &none)
            .expect("lost")
            .is_empty()
    );
    assert_eq!(
        a(&mem).review.turn,
        Some(ReviewTurn::Agent { decides: true })
    );
    assert!(
        mem.deferred
            .iter()
            .all(|e| !matches!(e, ExternalEffect::StartReviewer { .. }))
    );
}
