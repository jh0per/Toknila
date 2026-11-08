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
use crate::chains::{self, ChainSettings};
use crate::external::ExternalEffect;
use crate::projects::{self, PatchProject};
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
    assert_eq!(a(&mem).review.turn, Some(ReviewTurn::Merging));
    settle(&mut mem);
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
    settle(&mut mem);
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
    let settings = ChainSettings {
        review_mode: Some(ReviewMode::Both),
        reviewer: Some(" qa ".to_owned()),
        autostart: None,
    };
    chains::set_settings(&mut mem, "demo", None, settings).expect("chain");
    assert_eq!(mem.view.chain.review_mode, Some(ReviewMode::Both));
    assert_eq!(mem.view.chain.reviewer.as_deref(), Some("qa"));
    let patch = PatchProject {
        review_mode: Some(ReviewMode::Both),
        reviewer: Some(" qa ".to_owned()),
        ..PatchProject::default()
    };
    let p = projects::patch(&mut mem, "DEMO", patch).expect("project");
    assert_eq!(p.default_review_mode, ReviewMode::Both);
    assert_eq!(p.default_reviewer.as_deref(), Some("qa"));
    let clear = PatchProject {
        reviewer: Some(String::new()),
        ..PatchProject::default()
    };
    projects::patch(&mut mem, "DEMO", clear).expect("project");
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
            brief: crate::runner::RunBrief::default(),
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

// ---- slots and delivery ----

/// Project DEMO with process agents and a limit of 2, review by the agent, and three
/// independent cards (card 1..3) nobody has started.
fn three_cards() -> Mem {
    let mut mem = with_agents();
    mem.project.default_review_mode = ReviewMode::Agent;
    mem.project.max_parallel = 2;
    mem.view.chain.autostart = false;
    mem.view.edges.clear();
    mem.view.tickets.clear();
    for i in 1..=3 {
        let key = format!("DEMO-{i}").parse().expect("key");
        let draft = toknila_core::TicketDraft {
            title: format!("card {i}"),
            ..toknila_core::TicketDraft::default()
        };
        let t = Ticket::new_backlog(
            toknila_core::TicketId::new(),
            mem.view.chain.id,
            key,
            draft,
            mem.now,
        );
        mem.view.tickets.push(t);
    }
    let chain = mem.view.chain.id;
    crate::with_chain(&mut mem, chain, |_| Ok::<_, crate::AppError>(())).expect("settle");
    mem
}

fn card(mem: &Mem, n: usize) -> Ticket {
    mem.ticket_by_title(&format!("card {n}"))
}

#[test]
fn a_card_in_review_with_worker_and_reviewer_alive_holds_two_slots() {
    let mut mem = three_cards();
    for n in 1..=2 {
        let t = card(&mem, n);
        tickets::act(&mut mem, &t, TicketEvent::Request).expect("request");
    }
    settle(&mut mem);
    let one = card(&mem, 1);
    let event = AgentEvent::Finished {
        summary: "done".to_owned(),
    };
    agent::ticket_event(&mut mem, &one, event).expect("finished");
    settle(&mut mem);
    assert_eq!(card(&mem, 1).status, TicketStatus::Review);
    assert_eq!(
        live(&mem.runner).len(),
        3,
        "worker and reviewer of 1, worker of 2"
    );

    // Limit 2: the review card's two runners and the running card pass it, so a third card
    // that a person starts stays queued.
    let three = card(&mem, 3);
    tickets::act(&mut mem, &three, TicketEvent::Request).expect("request");
    settle(&mut mem);
    let three = card(&mem, 3);
    assert_eq!((three.status, three.queued), (TicketStatus::Ready, true));
    let positions = crate::schedule::queue_positions(&mem).expect("positions");
    assert_eq!(positions.get(&three.id), Some(&1));

    // Accepting card 1 stops both of its runners and frees the slots: card 3 starts.
    let one = card(&mem, 1);
    tickets::act(&mut mem, &one, TicketEvent::Accept { comment: None }).expect("accept");
    settle(&mut mem);
    assert_eq!(card(&mem, 1).status, TicketStatus::Merged);
    assert_eq!(card(&mem, 3).status, TicketStatus::Running);
}

#[test]
fn a_returns_comment_is_typed_to_the_live_worker_and_marked_delivered_once() {
    let mut mem = process_review(ReviewMode::Human);
    mem.runner.reach.set(crate::runner::RunnerState {
        hooks: true,
        idle: true,
        ..Default::default()
    });
    let comment = "fix the look".to_owned();
    act(&mut mem, TicketEvent::Return { comment });
    settle(&mut mem);
    assert_eq!(a(&mem).status, TicketStatus::Running);
    let written = mem.runner.written.borrow().clone();
    assert_eq!(written[0], b"# [instruction] fix the look");
    assert_eq!(written[1], b"\r");
    let delivered = mem
        .kinds()
        .iter()
        .filter(|k| *k == "comment.delivered")
        .count();
    assert_eq!(delivered, 1);
    let t = a(&mem);
    let claimed = crate::instructions::claim(&mut mem, &t).expect("claim");
    assert_eq!(claimed.count, 0, "nothing is left for the Stop hook");
}

#[test]
fn the_reviewers_remarks_wait_for_the_stop_hook_of_a_working_worker() {
    let mut mem = process_review(ReviewMode::Agent);
    mem.runner.reach.set(crate::runner::RunnerState {
        hooks: true,
        idle: false,
        ..Default::default()
    });
    verdict(&mut mem, Verdict::RequestChanges, "handle the empty list").expect("verdict");
    settle(&mut mem);
    assert!(
        mem.runner.written.borrow().is_empty(),
        "nothing typed mid-turn"
    );
    let t = a(&mem);
    let claimed = crate::instructions::claim(&mut mem, &t).expect("claim");
    assert_eq!(claimed.count, 1);
    assert!(claimed.reason.contains("handle the empty list"));
}

#[test]
fn a_recorded_verdict_stops_the_reviewer_and_its_slot_starts_the_next_card() {
    let mut mem = three_cards();
    mem.project.max_parallel = 3;
    mem.project.default_review_mode = ReviewMode::Both;
    for n in 1..=2 {
        let t = card(&mem, n);
        tickets::act(&mut mem, &t, TicketEvent::Request).expect("request");
    }
    settle(&mut mem);
    let one = card(&mem, 1);
    let finished = AgentEvent::Finished {
        summary: "done".to_owned(),
    };
    agent::ticket_event(&mut mem, &one, finished).expect("finished");
    settle(&mut mem);
    assert_eq!(
        live(&mem.runner).len(),
        3,
        "worker and reviewer of 1, worker of 2"
    );
    let three = card(&mem, 3);
    tickets::act(&mut mem, &three, TicketEvent::Request).expect("request");
    settle(&mut mem);
    assert!(card(&mem, 3).queued, "the three slots are taken");

    // The reviewer gives its verdict: its process is stopped, the worker's is not.
    let verdict = AgentEvent::ReviewVerdict {
        verdict: Verdict::Approve,
        comment: "fine".to_owned(),
    };
    agent::ticket_event(&mut mem, &one, verdict).expect("verdict");
    assert!(
        mem.deferred
            .iter()
            .any(|e| matches!(e, ExternalEffect::StopReviewer { ticket } if *ticket == one.id)),
        "the stop is queued for after the commit: {:?}",
        mem.deferred
    );
    settle(&mut mem);
    assert_eq!(
        mem.runner.stopped_roles.borrow().as_slice(),
        [(one.id, AgentRole::Reviewer)]
    );
    assert!(live(&mem.runner).contains(&AgentRole::Worker));
    assert_eq!(live(&mem.runner).len(), 2, "worker of 1, worker of 2");

    // The process is gone: the host reports its end, which frees the slot for card 3.
    let req = RunRequest {
        ticket: one.id,
        key: one.key.clone(),
        role: AgentRole::Reviewer,
        profile: "fake-reviewer".to_owned(),
        dir: "/ships/demo/demo/demo-1-a".to_owned(),
        brief: crate::runner::RunBrief::default(),
    };
    let end = RunEnded {
        run: run_of(&req),
        exit: ProcessExit {
            code: None,
            signal: Some("SIGTERM".to_owned()),
        },
        stopped: true,
    };
    runs::ended(&mut mem, end).expect("ended");
    settle(&mut mem);
    assert_eq!(card(&mem, 3).status, TicketStatus::Running);
    assert_eq!(
        card(&mem, 1).status,
        TicketStatus::Review,
        "the card itself waits for the person"
    );
}

fn nudges(mem: &Mem) -> usize {
    mem.deferred
        .iter()
        .filter(|e| matches!(e, ExternalEffect::NudgeReviewer { .. }))
        .count()
}

#[test]
fn a_reviewer_that_stops_twice_without_a_verdict_is_nudged_once_then_a_person_decides() {
    let mut mem = process_review(ReviewMode::Both);
    let t = a(&mem);
    let before = nudges(&mem);
    reviews::stopped(&mut mem, t.id, vec!["first".to_owned()]).expect("stopped");
    assert_eq!(nudges(&mem), before + 1, "one nudge for the host to type");
    assert_eq!(
        a(&mem).review.turn,
        Some(ReviewTurn::Agent { decides: false })
    );

    reviews::stopped(&mut mem, t.id, vec!["I could not run it".to_owned()]).expect("stopped");
    let t = a(&mem);
    assert_eq!(nudges(&mem), before + 1, "no second nudge");
    assert_eq!(t.review.turn, Some(ReviewTurn::Person));
    assert_eq!(
        t.review.note,
        Some(toknila_core::ReviewNote::NoVerdict {
            lines: vec!["I could not run it".to_owned()]
        })
    );
    assert!(
        mem.deferred
            .iter()
            .any(|e| matches!(e, ExternalEffect::StopReviewer { .. })),
        "the reviewer's runner is stopped"
    );
}

#[test]
fn a_stop_after_the_verdict_is_ignored_and_the_reviewer_can_be_run_again() {
    let mut mem = process_review(ReviewMode::Both);
    let t = a(&mem);
    verdict(&mut mem, Verdict::Approve, "fine").expect("verdict");
    let before = a(&mem);
    reviews::stopped(&mut mem, t.id, vec![]).expect("ignored");
    assert_eq!(a(&mem), before);

    // A reviewer that gave none: run it again from the person's turn.
    let mut mem = process_review(ReviewMode::Both);
    let t = a(&mem);
    reviews::stopped(&mut mem, t.id, vec![]).expect("nudge");
    reviews::stopped(&mut mem, t.id, vec![]).expect("hand over");
    let started = reviewer_requests(&mem).len();
    act(&mut mem, TicketEvent::RunReviewer);
    settle(&mut mem);
    let t = a(&mem);
    assert_eq!(t.review.turn, Some(ReviewTurn::Agent { decides: false }));
    assert_eq!(t.review.note, None);
    assert!(
        reviewer_requests(&mem).len() > started,
        "a new reviewer run"
    );
}
