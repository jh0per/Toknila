use super::*;
use crate::machine::review;
use crate::{ReviewMode, ReviewNote, ReviewTurn, Verdict, machine::TicketAction};

fn finished(mode: ReviewMode) -> TicketEvent {
    TicketEvent::Finished {
        summary: "done".to_owned(),
        open_threads: vec![],
        mode,
        check: false,
        max_rounds: 3,
    }
}

fn finished_with_check(mode: ReviewMode) -> TicketEvent {
    match finished(mode) {
        TicketEvent::Finished {
            summary,
            open_threads,
            mode,
            max_rounds,
            ..
        } => TicketEvent::Finished {
            summary,
            open_threads,
            mode,
            check: true,
            max_rounds,
        },
        other => other,
    }
}

fn check_done(passed: bool, mode: ReviewMode, max_rounds: u32) -> TicketEvent {
    TicketEvent::CheckDone {
        passed,
        output: "error: the thing failed".to_owned(),
        mode,
        max_rounds,
    }
}

fn in_review(turn: ReviewTurn) -> Ticket {
    let mut t = ticket_in(TicketStatus::Review);
    t.review.turn = Some(turn);
    t
}

fn verdict(verdict: Verdict, comment: &str) -> TicketEvent {
    TicketEvent::ReviewVerdict {
        verdict,
        comment: comment.to_owned(),
        items: vec![],
        max_rounds: 3,
    }
}

fn step(t: &Ticket, event: &TicketEvent) -> Transition {
    let tr = apply(t, event, NOW).unwrap_or_else(|e| panic!("{event:?}: {e}"));
    check_invariants(&tr.ticket).unwrap_or_else(|e| panic!("{event:?}: {e}"));
    tr
}

#[test]
fn a_finished_card_waits_for_whoever_the_mode_says() {
    let cases = [
        (ReviewMode::Human, ReviewTurn::Person, false),
        (ReviewMode::Agent, ReviewTurn::Agent { decides: true }, true),
        (ReviewMode::Both, ReviewTurn::Agent { decides: false }, true),
    ];
    for (mode, turn, reviewer_starts) in cases {
        let tr = step(&ticket_in(TicketStatus::Running), &finished(mode));
        assert_eq!(tr.ticket.status, TicketStatus::Review, "{mode:?}");
        assert_eq!(tr.ticket.review.turn, Some(turn), "{mode:?}");
        assert_eq!(
            tr.effects.contains(&Effect::StartReviewer),
            reviewer_starts,
            "{mode:?}"
        );
        assert!(!tr.effects.contains(&Effect::StopRunner), "{mode:?}");
    }
}

#[test]
fn in_agent_mode_an_approval_starts_the_merge_and_is_a_round() {
    let t = in_review(ReviewTurn::Agent { decides: true });
    let tr = step(&t, &verdict(Verdict::Approve, "looks right"));
    assert_eq!(tr.ticket.status, TicketStatus::Review);
    assert_eq!(tr.ticket.review.turn, Some(ReviewTurn::Merging));
    let round = &tr.ticket.review.rounds[0];
    assert_eq!(
        (round.number, round.reviewer, round.verdict),
        (1, Actor::Agent, Verdict::Approve)
    );
    assert_eq!(round.comment, "looks right");
    assert_eq!(tr.effects, [Effect::StopReviewer, Effect::Merge]);
}

#[test]
fn in_agent_mode_requested_changes_go_back_to_the_worker() {
    let t = in_review(ReviewTurn::Agent { decides: true });
    let tr = step(
        &t,
        &verdict(Verdict::RequestChanges, "handle the empty list"),
    );
    assert_eq!(tr.ticket.status, TicketStatus::Running);
    assert_eq!(tr.ticket.review.turn, None);
    assert_eq!(
        tr.effects,
        [
            Effect::StopReviewer,
            Effect::RecordRemarks {
                text: "handle the empty list".to_owned()
            },
            Effect::PrepareShip,
            Effect::EnsureRunner
        ]
    );
    assert!(
        !tr.effects.contains(&Effect::StopRunner),
        "the worker lives on"
    );
}

fn verdict_with_limit(v: Verdict, comment: &str, max_rounds: u32) -> TicketEvent {
    TicketEvent::ReviewVerdict {
        verdict: v,
        comment: comment.to_owned(),
        items: vec![],
        max_rounds,
    }
}

#[test]
fn in_both_mode_an_approval_hands_the_card_to_a_person() {
    let t = in_review(ReviewTurn::Agent { decides: false });
    let tr = step(&t, &verdict(Verdict::Approve, "see line 3"));
    assert_eq!(tr.ticket.status, TicketStatus::Review);
    assert_eq!(tr.ticket.review.turn, Some(ReviewTurn::Person));
    assert_eq!(tr.ticket.review.rounds.len(), 1);
    assert_eq!(tr.ticket.review.rounds[0].verdict, Verdict::Approve);
    assert_eq!(
        tr.effects,
        [Effect::StopReviewer],
        "only the finished reviewer goes; nothing else moves until the person acts"
    );
}

#[test]
fn in_both_mode_requested_changes_return_the_card_until_the_round_limit() {
    let t = in_review(ReviewTurn::Agent { decides: false });
    let tr = step(&t, &verdict(Verdict::RequestChanges, "see line 3"));
    assert_eq!(tr.ticket.status, TicketStatus::Running, "round 1 of 3");
    assert!(tr.effects.contains(&Effect::RecordRemarks {
        text: "see line 3".to_owned()
    }));

    // With the limit used up the same verdict is a round, and the card waits for the person.
    let tr = step(&t, &verdict_with_limit(Verdict::RequestChanges, "again", 0));
    assert_eq!(tr.ticket.status, TicketStatus::Review);
    assert_eq!(tr.ticket.review.turn, Some(ReviewTurn::Person));
    assert_eq!(tr.ticket.review.rounds.len(), 1);
    assert!(matches!(
        tr.effects[..],
        [Effect::StopReviewer, Effect::RecordReport { .. }]
    ));
}

#[test]
fn the_person_after_the_agent_adds_a_second_round() {
    let t = in_review(ReviewTurn::Agent { decides: false });
    let t = step(
        &t,
        &verdict_with_limit(Verdict::RequestChanges, "rename it", 0),
    )
    .ticket;
    let t = step(
        &t,
        &TicketEvent::Return {
            comment: "also fix the look".to_owned(),
        },
    )
    .ticket;
    assert_eq!(t.status, TicketStatus::Running);
    let rounds: Vec<_> = t
        .review
        .rounds
        .iter()
        .map(|r| (r.number, r.reviewer, r.verdict))
        .collect();
    assert_eq!(
        rounds,
        [
            (1, Actor::Agent, Verdict::RequestChanges),
            (2, Actor::Human, Verdict::RequestChanges)
        ]
    );
}

#[test]
fn a_verdict_needs_the_card_to_wait_for_the_agent() {
    let refused = [
        in_review(ReviewTurn::Person),
        ticket_in(TicketStatus::Review),
        ticket_in(TicketStatus::Running),
    ];
    for t in refused {
        assert!(
            matches!(
                apply(&t, &verdict(Verdict::Approve, ""), NOW),
                Err(TransitionError::NotAllowed { .. })
            ),
            "{:?}",
            t.status
        );
    }
}

#[test]
fn requested_changes_need_remarks() {
    let t = in_review(ReviewTurn::Agent { decides: true });
    assert_eq!(
        apply(&t, &verdict(Verdict::RequestChanges, "  "), NOW),
        Err(TransitionError::EmptyText("request_changes"))
    );
}

#[test]
fn a_person_can_decide_while_the_reviewer_is_still_looking() {
    let t = in_review(ReviewTurn::Agent { decides: true });
    let accepted = step(
        &t,
        &TicketEvent::Accept {
            comment: Some("fine".to_owned()),
        },
    );
    assert_eq!(accepted.ticket.review.turn, Some(ReviewTurn::Merging));
    assert!(accepted.effects.contains(&Effect::StopReviewer));
    let round = &accepted.ticket.review.rounds[0];
    assert_eq!(
        (round.reviewer, round.comment.as_str()),
        (Actor::Human, "fine")
    );

    let returned = step(
        &t,
        &TicketEvent::Return {
            comment: "no".to_owned(),
        },
    );
    assert_eq!(returned.ticket.status, TicketStatus::Running);
    assert!(returned.effects.contains(&Effect::StopReviewer));
    assert!(
        !returned.effects.contains(&Effect::StopRunner),
        "return goes to the worker's session"
    );
}

#[test]
fn a_reviewer_that_fails_hands_the_card_to_a_person() {
    let t = in_review(ReviewTurn::Agent { decides: true });
    let tr = step(
        &t,
        &TicketEvent::ReviewerFailed {
            detail: "exit 1".to_owned(),
        },
    );
    assert_eq!(tr.ticket.status, TicketStatus::Review);
    assert_eq!(tr.ticket.review.turn, Some(ReviewTurn::Person));
    assert!(tr.ticket.review.rounds.is_empty());
    assert!(tr.effects.contains(&Effect::StopReviewer));
    // Too late once a person has the card.
    assert!(matches!(
        apply(
            &tr.ticket,
            &TicketEvent::ReviewerFailed {
                detail: "again".to_owned()
            },
            NOW
        ),
        Err(TransitionError::NotAllowed { .. })
    ));
}

#[test]
fn leaving_review_any_other_way_clears_whose_turn_it_is() {
    let t = in_review(ReviewTurn::Agent { decides: false });
    let conflict = TicketEvent::Block {
        reason: BlockedReason::MergeConflict {
            detail: "x".to_owned(),
        },
    };
    let blocked = step(&t, &conflict);
    assert_eq!(blocked.ticket.review.turn, None);
    assert!(blocked.effects.contains(&Effect::StopReviewer));
    let back = step(&blocked.ticket, &TicketEvent::Unblock);
    assert_eq!(back.ticket.status, TicketStatus::Review);
    assert_eq!(back.ticket.review.turn, Some(ReviewTurn::Merging));

    let cancelled = step(
        &t,
        &TicketEvent::Cancel {
            reason: "no".to_owned(),
        },
    );
    assert_eq!(cancelled.ticket.review.turn, None);

    let mut merged = ticket_in(TicketStatus::Merged);
    merged.review.rounds.clear();
    let reopened = step(&merged, &TicketEvent::Reopen);
    assert_eq!(reopened.ticket.review.turn, Some(ReviewTurn::Person));
}

#[test]
fn every_recorded_verdict_stops_the_reviewer_and_never_the_worker() {
    for (decides, v, comment) in [
        (true, Verdict::Approve, ""),
        (true, Verdict::RequestChanges, "fix it"),
        (false, Verdict::Approve, "fine"),
        (false, Verdict::RequestChanges, "fix it"),
    ] {
        let t = in_review(ReviewTurn::Agent { decides });
        let tr = step(&t, &verdict(v, comment));
        assert!(
            tr.effects.contains(&Effect::StopReviewer),
            "decides {decides}, {v:?}"
        );
        assert!(!tr.effects.contains(&Effect::StopRunner), "{v:?}");
    }
}

#[test]
fn a_refused_verdict_stops_nothing() {
    let t = in_review(ReviewTurn::Agent { decides: true });
    assert!(apply(&t, &verdict(Verdict::RequestChanges, " "), NOW).is_err());
}

fn stopped(lines: &[&str]) -> TicketEvent {
    TicketEvent::ReviewerStopped {
        screen: lines.iter().map(|l| (*l).to_owned()).collect(),
    }
}

#[test]
fn a_reviewer_that_ends_a_turn_without_a_verdict_is_nudged_once() {
    let t = in_review(ReviewTurn::Agent { decides: false });
    let tr = step(&t, &stopped(&["thinking"]));
    assert_eq!(
        tr.ticket.review.turn,
        Some(ReviewTurn::Agent { decides: false })
    );
    assert!(tr.ticket.review.nudged);
    assert_eq!(tr.ticket.review.note, None);
    assert_eq!(
        tr.effects,
        [Effect::NudgeReviewer {
            message: review::NUDGE_MESSAGE.to_owned()
        }]
    );
    assert!(review::NUDGE_MESSAGE.contains("review_verdict"));
}

#[test]
fn a_second_stop_without_a_verdict_hands_the_card_to_a_person_with_a_note() {
    let mut t = in_review(ReviewTurn::Agent { decides: true });
    t.review.nudged = true;
    let screen: Vec<String> = (0..20).map(|i| format!("line {i}")).collect();
    let tr = step(&t, &TicketEvent::ReviewerStopped { screen });
    assert_eq!(tr.ticket.review.turn, Some(ReviewTurn::Person));
    let Some(ReviewNote::NoVerdict { lines }) = &tr.ticket.review.note else {
        panic!("a note");
    };
    assert_eq!(lines.len(), 15, "at most 15 lines");
    assert_eq!(lines.last().map(String::as_str), Some("line 19"));
    assert_eq!(tr.effects, [Effect::StopReviewer]);
}

#[test]
fn a_stop_after_the_verdict_or_outside_the_reviewers_turn_is_refused() {
    for turn in [ReviewTurn::Person, ReviewTurn::Merging] {
        let t = in_review(turn);
        assert!(apply(&t, &stopped(&[]), NOW).is_err(), "{turn:?}");
    }
    let running = ticket_in(TicketStatus::Running);
    assert!(apply(&running, &stopped(&[]), NOW).is_err());
}

#[test]
fn a_person_can_run_the_reviewer_again_only_after_it_gave_no_verdict() {
    let mut t = in_review(ReviewTurn::Person);
    assert!(
        apply(&t, &TicketEvent::RunReviewer, NOW).is_err(),
        "no note"
    );
    assert!(!TicketAction::allowed(&t).contains(&TicketAction::RunReviewer));
    t.review.note = Some(ReviewNote::NoVerdict { lines: vec![] });
    assert!(TicketAction::allowed(&t).contains(&TicketAction::RunReviewer));
    let tr = step(&t, &TicketEvent::RunReviewer);
    assert_eq!(
        tr.ticket.review.turn,
        Some(ReviewTurn::Agent { decides: false })
    );
    assert_eq!(
        (tr.ticket.review.nudged, tr.ticket.review.note),
        (false, None)
    );
    assert_eq!(tr.effects, [Effect::StartReviewer]);
}

#[test]
fn a_persons_decision_clears_the_note() {
    let mut t = in_review(ReviewTurn::Person);
    t.review.note = Some(ReviewNote::NoVerdict { lines: vec![] });
    t.review.nudged = true;
    let accept = TicketAction::Accept
        .event(None, Actor::Human)
        .expect("event");
    let tr = step(&t, &accept);
    assert_eq!(
        (tr.ticket.review.nudged, tr.ticket.review.note),
        (false, None)
    );
}

#[test]
fn a_reviewer_that_exits_gets_the_run_again_action_too() {
    let t = in_review(ReviewTurn::Agent { decides: false });
    let failed = TicketEvent::ReviewerFailed {
        detail: "gone".to_owned(),
    };
    let tr = step(&t, &failed);
    assert!(tr.ticket.review.note.is_some());
    assert!(TicketAction::allowed(&tr.ticket).contains(&TicketAction::RunReviewer));
}

fn after_hand_in_with_check(mode: ReviewMode) -> Ticket {
    step(
        &ticket_in(TicketStatus::Running),
        &finished_with_check(mode),
    )
    .ticket
}

#[test]
fn a_hand_in_with_a_check_waits_for_the_check_before_anyone_reviews() {
    let tr = step(
        &ticket_in(TicketStatus::Running),
        &finished_with_check(ReviewMode::Both),
    );
    assert_eq!(tr.ticket.status, TicketStatus::Review);
    assert_eq!(tr.ticket.review.turn, Some(ReviewTurn::Check));
    assert!(tr.effects.contains(&Effect::RunCheck));
    assert!(
        !tr.effects.contains(&Effect::StartReviewer),
        "not before the check passes"
    );
}

#[test]
fn a_failing_check_returns_the_card_to_the_worker_with_the_output_as_a_round() {
    let t = after_hand_in_with_check(ReviewMode::Both);
    let tr = step(&t, &check_done(false, ReviewMode::Both, 3));
    assert_eq!(tr.ticket.status, TicketStatus::Running);
    assert_eq!(tr.ticket.review.turn, None);
    let round = &tr.ticket.review.rounds[0];
    assert_eq!(
        (round.number, round.reviewer, round.verdict),
        (1, Actor::System, Verdict::RequestChanges)
    );
    assert!(round.comment.contains("the thing failed"));
    let sent = tr.effects.iter().find_map(|e| match e {
        Effect::RecordCheckOutput { text } => Some(text.clone()),
        _ => None,
    });
    let sent = sent.expect("the output goes to the worker");
    assert!(
        sent.contains("round 1 of 3") && sent.contains("the thing failed"),
        "{sent}"
    );
    assert!(tr.effects.contains(&Effect::EnsureRunner));
    assert!(!tr.effects.contains(&Effect::StartReviewer));
}

#[test]
fn a_passing_check_goes_on_to_whoever_the_mode_says() {
    for (mode, turn) in [
        (ReviewMode::Human, ReviewTurn::Person),
        (ReviewMode::Both, ReviewTurn::Agent { decides: false }),
        (ReviewMode::Agent, ReviewTurn::Agent { decides: true }),
    ] {
        let t = after_hand_in_with_check(mode);
        let tr = step(&t, &check_done(true, mode, 3));
        assert_eq!(tr.ticket.status, TicketStatus::Review, "{mode:?}");
        assert_eq!(tr.ticket.review.turn, Some(turn), "{mode:?}");
        assert_eq!(
            tr.effects.contains(&Effect::StartReviewer),
            mode != ReviewMode::Human
        );
        let round = &tr.ticket.review.rounds[0];
        assert_eq!(
            (round.reviewer, round.verdict),
            (Actor::System, Verdict::Approve)
        );
    }
}

#[test]
fn after_the_round_limit_a_failing_check_leaves_the_card_to_the_person_with_the_history() {
    let mut t = after_hand_in_with_check(ReviewMode::Both);
    for round in 1..=2 {
        t = step(&t, &check_done(false, ReviewMode::Both, 2)).ticket;
        assert_eq!(t.status, TicketStatus::Running, "round {round}");
        t = step(&t, &finished_with_check(ReviewMode::Both)).ticket;
    }
    let tr = step(&t, &check_done(false, ReviewMode::Both, 2));
    assert_eq!(tr.ticket.status, TicketStatus::Review);
    assert_eq!(tr.ticket.review.turn, Some(ReviewTurn::Person));
    assert_eq!(tr.ticket.review.rounds.len(), 3, "the whole history");
    assert!(
        tr.effects
            .iter()
            .any(|e| matches!(e, Effect::RecordReport { summary } if summary.contains("2 rounds")))
    );
    assert!(
        !tr.effects.contains(&Effect::EnsureRunner),
        "nothing goes back to the worker"
    );
}

#[test]
fn a_persons_decision_starts_the_round_count_again() {
    let mut t = after_hand_in_with_check(ReviewMode::Human);
    t = step(&t, &check_done(false, ReviewMode::Human, 1)).ticket;
    assert_eq!(t.review.auto_returns(), 1);
    t = step(&t, &finished_with_check(ReviewMode::Human)).ticket;
    t = step(&t, &check_done(true, ReviewMode::Human, 1)).ticket;
    t = step(
        &t,
        &TicketEvent::Return {
            comment: "more".to_owned(),
        },
    )
    .ticket;
    assert_eq!(t.review.auto_returns(), 0);
}

#[test]
fn the_check_result_is_heard_only_while_the_card_is_being_checked() {
    for t in [
        in_review(ReviewTurn::Person),
        ticket_in(TicketStatus::Running),
    ] {
        assert!(apply(&t, &check_done(true, ReviewMode::Human, 3), NOW).is_err());
    }
    let lost = TicketEvent::CheckLost {
        detail: "no shell".to_owned(),
    };
    let t = after_hand_in_with_check(ReviewMode::Both);
    let tr = step(&t, &lost);
    assert_eq!(tr.ticket.review.turn, Some(ReviewTurn::Person));
    // A person cannot accept or return a card while its check runs.
    for event in [
        TicketEvent::Accept { comment: None },
        TicketEvent::Return {
            comment: "x".to_owned(),
        },
    ] {
        assert!(apply(&t, &event, NOW).is_err(), "{event:?}");
    }
}

fn with_acceptance(turn: ReviewTurn) -> Ticket {
    let mut t = in_review(turn);
    t.acceptance = vec!["first holds".to_owned(), "second holds".to_owned()];
    t
}

fn items(outcomes: &[(u32, crate::ItemOutcome)]) -> Vec<crate::ItemVerdict> {
    outcomes
        .iter()
        .map(|(item, outcome)| crate::ItemVerdict {
            item: *item,
            outcome: *outcome,
            basis: crate::ItemBasis::Ran,
            note: "seen".to_owned(),
        })
        .collect()
}

fn judged(v: Verdict, comment: &str, items: Vec<crate::ItemVerdict>) -> TicketEvent {
    TicketEvent::ReviewVerdict {
        verdict: v,
        comment: comment.to_owned(),
        items,
        max_rounds: 3,
    }
}

#[test]
fn the_verdict_must_judge_each_acceptance_item_pass_or_fail() {
    use crate::ItemOutcome::{Fail, Pass};
    let t = with_acceptance(ReviewTurn::Agent { decides: true });
    let refused = [
        judged(Verdict::Approve, "", vec![]),
        judged(Verdict::Approve, "", items(&[(1, Pass)])),
        judged(Verdict::Approve, "", items(&[(1, Pass), (1, Pass)])),
        judged(Verdict::Approve, "", items(&[(1, Pass), (3, Pass)])),
        judged(Verdict::Approve, "", items(&[(1, Pass), (2, Fail)])),
    ];
    for event in refused {
        assert!(
            matches!(apply(&t, &event, NOW), Err(TransitionError::Items(_))),
            "{event:?}"
        );
    }
    let tr = step(
        &t,
        &judged(Verdict::Approve, "", items(&[(2, Pass), (1, Pass)])),
    );
    assert_eq!(tr.ticket.review.turn, Some(ReviewTurn::Merging));
    assert_eq!(
        tr.ticket.review.rounds[0].comment,
        "PASS 1. first holds - seen [ran]\nPASS 2. second holds - seen [ran]"
    );
}

#[test]
fn a_failing_item_returns_the_card_with_the_item_lines_as_the_remarks() {
    use crate::ItemOutcome::{Fail, Pass};
    let t = with_acceptance(ReviewTurn::Agent { decides: true });
    let tr = step(
        &t,
        &judged(Verdict::RequestChanges, "", items(&[(1, Pass), (2, Fail)])),
    );
    assert_eq!(tr.ticket.status, TicketStatus::Running);
    assert!(tr.effects.contains(&Effect::RecordRemarks {
        text: "PASS 1. first holds - seen [ran]\nFAIL 2. second holds - seen [ran]".to_owned()
    }));
}
