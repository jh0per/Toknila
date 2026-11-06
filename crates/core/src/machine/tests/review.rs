use super::*;
use crate::{ReviewMode, ReviewTurn, Verdict};

fn finished(mode: ReviewMode) -> TicketEvent {
    TicketEvent::Finished {
        summary: "done".to_owned(),
        open_threads: vec![],
        mode,
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

#[test]
fn in_both_mode_the_agents_verdict_hands_the_card_to_a_person() {
    for v in [Verdict::Approve, Verdict::RequestChanges] {
        let t = in_review(ReviewTurn::Agent { decides: false });
        let tr = step(&t, &verdict(v, "see line 3"));
        assert_eq!(tr.ticket.status, TicketStatus::Review);
        assert_eq!(tr.ticket.review.turn, Some(ReviewTurn::Person));
        assert_eq!(tr.ticket.review.rounds.len(), 1);
        assert_eq!(tr.ticket.review.rounds[0].verdict, v);
        assert_eq!(
            tr.effects,
            [Effect::StopReviewer],
            "only the finished reviewer goes; nothing else moves until the person acts"
        );
    }
}

#[test]
fn the_person_after_the_agent_adds_a_second_round() {
    let t = in_review(ReviewTurn::Agent { decides: false });
    let t = step(&t, &verdict(Verdict::RequestChanges, "rename it")).ticket;
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
