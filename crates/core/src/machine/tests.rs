use proptest::prelude::*;

use super::*;
use crate::{
    Actor, BlockedReason, ChainId, NodeRef, StalePolicy, TicketDraft, TicketId, TicketKey, Wildcard,
};

const T0: Timestamp = Timestamp(1_000);
const NOW: Timestamp = Timestamp(5_000);

fn ticket_in(status: TicketStatus) -> Ticket {
    use TicketStatus as S;
    let status_reason = match status {
        S::Waiting => Some(StatusReason::Waiting(WaitingReason::Silent)),
        S::Blocked => Some(StatusReason::Blocked {
            reason: BlockedReason::Manual {
                text: "key".to_owned(),
            },
            resume_to: S::Running,
            waiting: None,
        }),
        S::Cancelled => Some(StatusReason::Cancelled {
            text: "dup".to_owned(),
        }),
        S::Proposed => Some(StatusReason::Proposed),
        _ => None,
    };
    let wildcard = (status == S::Proposed).then(|| Wildcard {
        from: Some("DEMO-9".parse().expect("key")),
        proposed_by: Actor::Agent,
        after: vec![NodeRef::Ticket(TicketId::new())],
        before: vec![NodeRef::Ticket(TicketId::new())],
        approved: false,
    });
    Ticket {
        id: TicketId::new(),
        chain_id: ChainId::new(),
        key: "DEMO-1".parse::<TicketKey>().expect("key"),
        title: "t".to_owned(),
        prompt: String::new(),
        acceptance: Vec::new(),
        status,
        status_reason,
        status_since: T0,
        paused: false,
        queued: false,
        profile: None,
        review_mode: None,
        reviewer: None,
        branch: None,
        needs_secrets: false,
        design_first: false,
        tokens_used: 0,
        wildcard,
        done_out_of_order: None,
        pr: None,
        review: crate::Review::default(),
        pos: None,
        size: None,
    }
}

fn manual() -> BlockedReason {
    BlockedReason::Manual {
        text: "чекаємо ключ API".to_owned(),
    }
}

fn ok(from: TicketStatus, event: TicketEvent) -> Transition {
    let t = ticket_in(from);
    let tr = apply(&t, &event, NOW).unwrap_or_else(|e| panic!("{from:?} + {event:?}: {e}"));
    check_invariants(&tr.ticket).unwrap_or_else(|e| panic!("{from:?} + {event:?}: {e}"));
    tr
}

// ---- one test per row of the transition table ----

#[test]
fn backlog_to_ready_when_deps_satisfied() {
    let tr = ok(TicketStatus::Backlog, TicketEvent::DepsSatisfied);
    assert_eq!(tr.ticket.status, TicketStatus::Ready);
    assert_eq!(tr.ticket.status_since, NOW);
    assert!(tr.effects.is_empty());
}

#[test]
fn paused_backlog_does_not_become_ready() {
    let mut t = ticket_in(TicketStatus::Backlog);
    t.paused = true;
    assert_eq!(
        apply(&t, &TicketEvent::DepsSatisfied, NOW),
        Err(TransitionError::Paused)
    );
}

#[test]
fn ready_back_to_backlog_when_deps_unsatisfied() {
    assert_eq!(
        ok(TicketStatus::Ready, TicketEvent::DepsUnsatisfied)
            .ticket
            .status,
        TicketStatus::Backlog
    );
}

#[test]
fn ready_to_running_on_start() {
    let tr = ok(TicketStatus::Ready, TicketEvent::Start);
    assert_eq!(tr.ticket.status, TicketStatus::Running);
    assert_eq!(tr.effects, vec![Effect::PrepareShip, Effect::EnsureRunner]);
}

#[test]
fn a_request_queues_a_ready_card_without_moving_it() {
    let tr = ok(TicketStatus::Ready, TicketEvent::Request);
    assert_eq!(tr.ticket.status, TicketStatus::Ready);
    assert!(tr.ticket.queued);
    assert_eq!(tr.ticket.status_since, T0);
    assert!(tr.effects.is_empty());
}

#[test]
fn a_second_request_is_refused() {
    let mut t = ticket_in(TicketStatus::Ready);
    t.queued = true;
    assert_eq!(
        apply(&t, &TicketEvent::Request, NOW),
        Err(TransitionError::AlreadyQueued)
    );
}

#[test]
fn only_a_ready_card_can_be_requested() {
    let err = apply(
        &ticket_in(TicketStatus::Running),
        &TicketEvent::Request,
        NOW,
    );
    assert_eq!(
        err.map_err(|e| e.to_string()),
        Err("cannot start a ticket in Running".to_owned())
    );
}

#[test]
fn leaving_ready_clears_the_queued_flag() {
    for event in [
        TicketEvent::Start,
        TicketEvent::DepsUnsatisfied,
        TicketEvent::Pause,
        TicketEvent::Cancel {
            reason: "r".to_owned(),
        },
    ] {
        let mut t = ticket_in(TicketStatus::Ready);
        t.queued = true;
        let tr = apply(&t, &event, NOW).unwrap_or_else(|e| panic!("{event:?}: {e}"));
        assert!(!tr.ticket.queued, "{event:?} kept the flag");
    }
}

#[test]
fn running_to_waiting_on_ask_human() {
    let reason = WaitingReason::Question {
        text: "який фреймворк?".to_owned(),
    };
    let tr = ok(
        TicketStatus::Running,
        TicketEvent::AskHuman {
            reason: reason.clone(),
        },
    );
    assert_eq!(tr.ticket.status, TicketStatus::Waiting);
    assert_eq!(tr.ticket.status_reason, Some(StatusReason::Waiting(reason)));
}

fn waiting_with(reason: WaitingReason) -> Ticket {
    let mut t = ticket_in(TicketStatus::Waiting);
    t.status_reason = Some(StatusReason::Waiting(reason));
    t
}

#[test]
fn a_question_replaces_the_silent_reason() {
    let reason = WaitingReason::Question {
        text: "which flow?".to_owned(),
    };
    let event = TicketEvent::AskHuman {
        reason: reason.clone(),
    };
    let tr = apply(&waiting_with(WaitingReason::Silent), &event, NOW).expect("replaces");
    assert_eq!(tr.ticket.status, TicketStatus::Waiting);
    assert_eq!(tr.ticket.status_reason, Some(StatusReason::Waiting(reason)));
}

#[test]
fn silence_never_replaces_an_open_question() {
    let open = WaitingReason::Question {
        text: "which flow?".to_owned(),
    };
    let event = TicketEvent::AskHuman {
        reason: WaitingReason::Silent,
    };
    let tr = apply(&waiting_with(open.clone()), &event, NOW).expect("kept");
    assert_eq!(tr.ticket.status, TicketStatus::Waiting);
    assert_eq!(tr.ticket.status_reason, Some(StatusReason::Waiting(open)));
}

#[test]
fn a_second_question_is_still_refused() {
    let open = WaitingReason::Question {
        text: "first".to_owned(),
    };
    let event = TicketEvent::AskHuman {
        reason: WaitingReason::Question {
            text: "second".to_owned(),
        },
    };
    assert!(apply(&waiting_with(open), &event, NOW).is_err());
}

#[test]
fn waiting_to_running_when_instruction_delivered() {
    let tr = ok(TicketStatus::Waiting, TicketEvent::InstructionDelivered);
    assert_eq!(tr.ticket.status, TicketStatus::Running);
    assert_eq!(tr.ticket.status_reason, None);
}

#[test]
fn waiting_to_running_when_the_agent_is_active_again() {
    let tr = ok(TicketStatus::Waiting, TicketEvent::AgentActive);
    assert_eq!(tr.ticket.status, TicketStatus::Running);
    assert_eq!(tr.ticket.status_reason, None);
    assert!(tr.effects.is_empty());
}

#[test]
fn waiting_to_running_at_once_when_a_person_answers() {
    for reason in [
        WaitingReason::Question { text: "q".into() },
        WaitingReason::Permission {
            text: "p".into(),
            detail: None,
        },
        WaitingReason::Silent,
    ] {
        let mut t = ticket_in(TicketStatus::Waiting);
        t.status_reason = Some(StatusReason::Waiting(reason));
        let tr = apply(&t, &TicketEvent::Answered, Timestamp(9)).expect("allowed");
        assert_eq!(tr.ticket.status, TicketStatus::Running);
        assert_eq!(tr.ticket.status_reason, None);
        assert_eq!(tr.ticket.status_since, Timestamp(9));
        assert!(tr.effects.is_empty());
    }
}

#[test]
fn an_answer_changes_nothing_outside_waiting() {
    for from in [
        TicketStatus::Running,
        TicketStatus::Review,
        TicketStatus::Blocked,
        TicketStatus::Ready,
    ] {
        let err =
            apply(&ticket_in(from), &TicketEvent::Answered, Timestamp(1)).expect_err("refused");
        assert!(
            matches!(
                err,
                TransitionError::NotAllowed {
                    event: "answered",
                    ..
                }
            ),
            "{from:?}"
        );
    }
}

#[test]
fn agent_activity_changes_nothing_outside_waiting() {
    for from in [
        TicketStatus::Running,
        TicketStatus::Review,
        TicketStatus::Blocked,
    ] {
        let t = ticket_in(from);
        let err = apply(&t, &TicketEvent::AgentActive, Timestamp(1)).expect_err("refused");
        assert!(
            matches!(err, TransitionError::NotAllowed { .. }),
            "{from:?}"
        );
    }
}

#[test]
fn running_to_review_on_finished_records_report() {
    let tr = ok(
        TicketStatus::Running,
        TicketEvent::Finished {
            summary: "зроблено X".to_owned(),
            open_threads: vec![],
            mode: crate::ReviewMode::Human,
            check: false,
            max_rounds: 3,
        },
    );
    assert_eq!(tr.ticket.status, TicketStatus::Review);
    assert_eq!(
        tr.effects,
        vec![
            Effect::RecordReport {
                summary: "зроблено X".to_owned()
            },
            Effect::StopWorker
        ]
    );
}

#[test]
fn running_and_waiting_block_and_remember_where_they_were() {
    for from in [TicketStatus::Running, TicketStatus::Waiting] {
        let tr = ok(from, TicketEvent::Block { reason: manual() });
        assert_eq!(tr.ticket.status, TicketStatus::Blocked);
        assert_eq!(
            tr.ticket.status_reason,
            Some(StatusReason::Blocked {
                reason: manual(),
                resume_to: from,
                waiting: (from == TicketStatus::Waiting).then_some(WaitingReason::Silent),
            })
        );
    }
}

#[test]
fn crash_blocks_and_stops_runner() {
    let tr = ok(
        TicketStatus::Running,
        TicketEvent::Block {
            reason: BlockedReason::Crashed {
                detail: "exit 101".to_owned(),
            },
        },
    );
    assert_eq!(tr.ticket.status, TicketStatus::Blocked);
    assert_eq!(tr.effects, vec![Effect::StopRunner]);
}

#[test]
fn review_blocks_only_on_merge_conflict() {
    let conflict = BlockedReason::MergeConflict {
        detail: "src/lib.rs".to_owned(),
    };
    assert_eq!(
        ok(
            TicketStatus::Review,
            TicketEvent::Block { reason: conflict }
        )
        .ticket
        .status,
        TicketStatus::Blocked
    );
    let t = ticket_in(TicketStatus::Review);
    assert!(matches!(
        apply(&t, &TicketEvent::Block { reason: manual() }, NOW),
        Err(TransitionError::NotAllowed { .. })
    ));
}

#[test]
fn unblock_returns_to_previous_status() {
    for from in [
        TicketStatus::Running,
        TicketStatus::Waiting,
        TicketStatus::Review,
    ] {
        let reason = if from == TicketStatus::Review {
            BlockedReason::MergeConflict {
                detail: "x".to_owned(),
            }
        } else {
            manual()
        };
        let blocked = apply(&ticket_in(from), &TicketEvent::Block { reason }, NOW).expect("block");
        let tr = apply(&blocked.ticket, &TicketEvent::Unblock, Timestamp(9_000)).expect("unblock");
        assert_eq!(tr.ticket.status, from);
        assert_eq!(tr.ticket.status_since, Timestamp(9_000));
        check_invariants(&tr.ticket).expect("invariants");
    }
}

#[test]
fn unblock_to_waiting_restores_the_open_question() {
    let question = WaitingReason::Question {
        text: "який фреймворк?".to_owned(),
    };
    let waiting = ok(
        TicketStatus::Running,
        TicketEvent::AskHuman {
            reason: question.clone(),
        },
    );
    let blocked = apply(
        &waiting.ticket,
        &TicketEvent::Block { reason: manual() },
        NOW,
    )
    .expect("block");
    let back = apply(&blocked.ticket, &TicketEvent::Unblock, NOW).expect("unblock");
    assert_eq!(
        back.ticket.status_reason,
        Some(StatusReason::Waiting(question))
    );
    check_invariants(&back.ticket).expect("invariants");
}

#[test]
fn review_back_to_running_on_return_with_comment() {
    let tr = ok(
        TicketStatus::Review,
        TicketEvent::Return {
            comment: "додай тест".to_owned(),
        },
    );
    assert_eq!(tr.ticket.status, TicketStatus::Running);
    assert_eq!(
        tr.effects,
        vec![
            Effect::RecordInstruction {
                text: "додай тест".to_owned()
            },
            Effect::PrepareShip,
            Effect::EnsureRunner
        ]
    );
    let t = ticket_in(TicketStatus::Review);
    assert_eq!(
        apply(
            &t,
            &TicketEvent::Return {
                comment: "  ".to_owned()
            },
            NOW
        ),
        Err(TransitionError::EmptyText("return"))
    );
}

#[test]
fn unblock_to_running_prepares_the_ship_again() {
    let blocked = ok(
        TicketStatus::Running,
        TicketEvent::Block {
            reason: BlockedReason::ShipFailed {
                detail: "no git".to_owned(),
            },
        },
    );
    assert_eq!(blocked.effects, vec![Effect::StopRunner]);
    let tr = apply(&blocked.ticket, &TicketEvent::Unblock, NOW).expect("unblock");
    assert_eq!(tr.effects, vec![Effect::PrepareShip, Effect::EnsureRunner]);
}

#[test]
fn unblock_after_no_working_dir_starts_the_ship_and_runner_again() {
    let blocked = ok(
        TicketStatus::Running,
        TicketEvent::Block {
            reason: BlockedReason::NoWorkingDir,
        },
    );
    assert_eq!(blocked.effects, vec![Effect::StopRunner]);
    let tr = apply(&blocked.ticket, &TicketEvent::Unblock, NOW).expect("unblock");
    assert_eq!(tr.effects, vec![Effect::PrepareShip, Effect::EnsureRunner]);
}

#[test]
fn ship_ready_tells_an_active_ticket_its_branch_without_moving_it() {
    for from in [TicketStatus::Running, TicketStatus::Waiting] {
        let tr = ok(
            from,
            TicketEvent::ShipReady {
                branch: "ship/demo/demo-1-a".to_owned(),
            },
        );
        assert_eq!(tr.ticket.status, from);
        assert_eq!(tr.ticket.branch.as_deref(), Some("ship/demo/demo-1-a"));
        assert_eq!(tr.ticket.status_since, T0);
        assert!(tr.effects.is_empty());
    }
    let late = apply(
        &ticket_in(TicketStatus::Review),
        &TicketEvent::ShipReady {
            branch: "b".to_owned(),
        },
        NOW,
    );
    assert!(matches!(late, Err(TransitionError::NotAllowed { .. })));
}

#[test]
fn cancel_retires_the_ship() {
    for from in [TicketStatus::Running, TicketStatus::Review] {
        let tr = ok(
            from,
            TicketEvent::Cancel {
                reason: "dup".to_owned(),
            },
        );
        assert!(tr.effects.contains(&Effect::RetireShip), "{from:?}");
        assert!(!tr.effects.contains(&Effect::RemoveShip), "{from:?}");
        assert!(!tr.effects.contains(&Effect::PrepareShip), "{from:?}");
    }
}

#[test]
fn merged_to_released_when_milestone_released() {
    assert_eq!(
        ok(TicketStatus::Merged, TicketEvent::Released)
            .ticket
            .status,
        TicketStatus::Released
    );
}

#[test]
fn merged_back_to_review_on_reopen() {
    let tr = ok(TicketStatus::Merged, TicketEvent::Reopen);
    assert_eq!(tr.ticket.status, TicketStatus::Review);
    assert_eq!(tr.effects, vec![Effect::DependencyReopened]);
}

#[test]
fn pause_sends_active_tickets_to_backlog() {
    use TicketStatus as S;
    for from in [S::Backlog, S::Ready, S::Running, S::Waiting, S::Blocked] {
        let tr = ok(from, TicketEvent::Pause);
        assert_eq!(tr.ticket.status, S::Backlog);
        assert!(tr.ticket.paused);
        assert_eq!(tr.ticket.status_reason, None);
        let stops = tr.effects.contains(&Effect::StopRunner);
        assert_eq!(
            stops,
            matches!(from, S::Running | S::Waiting | S::Blocked),
            "{from:?}"
        );
    }
}

#[test]
fn resume_clears_pause_and_rechecks() {
    let paused = ok(TicketStatus::Running, TicketEvent::Pause).ticket;
    let tr = apply(&paused, &TicketEvent::Resume, NOW).expect("resume");
    assert!(!tr.ticket.paused);
    assert_eq!(tr.ticket.status, TicketStatus::Backlog);
    assert_eq!(tr.effects, vec![Effect::RecheckReadiness]);
    assert_eq!(
        apply(&tr.ticket, &TicketEvent::Resume, NOW),
        Err(TransitionError::NotPaused)
    );
}

#[test]
fn cancel_from_anything_but_done() {
    use TicketStatus as S;
    for from in [
        S::Backlog,
        S::Ready,
        S::Running,
        S::Waiting,
        S::Blocked,
        S::Review,
    ] {
        let tr = ok(
            from,
            TicketEvent::Cancel {
                reason: "won't do".to_owned(),
            },
        );
        assert_eq!(tr.ticket.status, S::Cancelled);
        assert!(tr.effects.contains(&Effect::DependencySatisfied));
    }
    for from in [S::Merged, S::Released, S::Cancelled] {
        let r = apply(
            &ticket_in(from),
            &TicketEvent::Cancel {
                reason: "x".to_owned(),
            },
            NOW,
        );
        assert!(
            matches!(r, Err(TransitionError::NotAllowed { .. })),
            "{from:?}"
        );
    }
}

// ---- wildcard cards ----

#[test]
fn a_proposed_card_carries_its_mark_and_is_not_in_the_plan() {
    let mark = Wildcard {
        from: Some("DEMO-1".parse().expect("key")),
        proposed_by: Actor::Agent,
        after: vec![],
        before: vec![],
        approved: true,
    };
    let draft = TicketDraft {
        title: "token refresh".to_owned(),
        ..TicketDraft::default()
    };
    let key = "DEMO-5".parse().expect("key");
    let t = Ticket::new_proposed(TicketId::new(), ChainId::new(), key, draft, mark, T0);
    assert_eq!(
        (t.status, t.status_reason.clone()),
        (TicketStatus::Proposed, Some(StatusReason::Proposed))
    );
    let w = t.wildcard.clone().expect("mark");
    assert!(!w.approved, "a new card is never approved");
    assert_eq!(w.from.map(|k| k.to_string()).as_deref(), Some("DEMO-1"));
    assert!(!t.in_plan());
    check_invariants(&t).expect("invariants");
}

#[test]
fn approve_moves_a_card_to_backlog_and_hands_over_its_edges() {
    let card = ticket_in(TicketStatus::Proposed);
    let pending = card.wildcard.clone().expect("mark");
    let tr = apply(&card, &TicketEvent::Approve { by: Actor::Human }, NOW).expect("approve");
    check_invariants(&tr.ticket).expect("invariants");
    assert_eq!(
        (tr.ticket.status, tr.ticket.status_reason.clone()),
        (TicketStatus::Backlog, None)
    );
    assert_eq!(
        tr.effects,
        [Effect::AddProposedEdges {
            after: pending.after,
            before: pending.before,
        }]
    );
    let w = tr.ticket.wildcard.clone().expect("the mark stays");
    assert!(w.approved && w.after.is_empty() && w.before.is_empty());
    assert_eq!(w.from.map(|k| k.to_string()).as_deref(), Some("DEMO-9"));
    assert!(tr.ticket.in_plan());
    assert_eq!(tr.ticket.status_since, NOW);
}

#[test]
fn reject_cancels_a_card_with_the_reason_and_keeps_it_out_of_the_plan() {
    let tr = ok(
        TicketStatus::Proposed,
        TicketEvent::Reject {
            reason: "not now".to_owned(),
            by: Actor::Human,
        },
    );
    assert_eq!(
        (tr.ticket.status, tr.ticket.status_reason.clone()),
        (
            TicketStatus::Cancelled,
            Some(StatusReason::Cancelled {
                text: "not now".to_owned()
            })
        )
    );
    assert!(tr.effects.is_empty(), "nothing waits for a proposed card");
    assert!(!tr.ticket.wildcard.as_ref().expect("mark").approved);
    assert!(!tr.ticket.in_plan());
    let blank = TicketEvent::Reject {
        reason: " ".to_owned(),
        by: Actor::Human,
    };
    assert_eq!(
        apply(&ticket_in(TicketStatus::Proposed), &blank, NOW),
        Err(TransitionError::EmptyText("reject"))
    );
}

#[test]
fn an_agent_cannot_approve_or_reject_a_card() {
    let card = ticket_in(TicketStatus::Proposed);
    for actor in [Actor::Agent, Actor::System] {
        assert_eq!(
            apply(&card, &TicketEvent::Approve { by: actor }, NOW),
            Err(TransitionError::PersonOnly {
                event: "approve",
                actor
            })
        );
        let reject = TicketEvent::Reject {
            reason: "no".to_owned(),
            by: actor,
        };
        assert_eq!(
            apply(&card, &reject, NOW),
            Err(TransitionError::PersonOnly {
                event: "reject",
                actor
            })
        );
    }
    assert_eq!(
        TransitionError::PersonOnly {
            event: "approve",
            actor: Actor::Agent
        }
        .to_string(),
        "only a person can approve a card; this came from Agent"
    );
}

#[test]
fn only_proposed_cards_can_be_approved() {
    let r = apply(
        &ticket_in(TicketStatus::Backlog),
        &TicketEvent::Approve { by: Actor::Human },
        NOW,
    );
    assert_eq!(
        r.map_err(|e| e.to_string()),
        Err("cannot approve a ticket in Backlog".to_owned())
    );
}

#[test]
fn a_proposed_card_without_its_mark_breaks_the_invariants() {
    let mut t = ticket_in(TicketStatus::Proposed);
    t.wildcard = None;
    assert!(check_invariants(&t).is_err());
    let mut t = ticket_in(TicketStatus::Proposed);
    t.status_reason = None;
    assert!(check_invariants(&t).is_err());
    let mut t = ticket_in(TicketStatus::Proposed);
    if let Some(w) = t.wildcard.as_mut() {
        w.approved = true;
    }
    assert!(check_invariants(&t).is_err());
}

// ---- the whole matrix: exactly the table's pairs are allowed ----

fn sample_events() -> Vec<TicketEvent> {
    vec![
        TicketEvent::DepsSatisfied,
        TicketEvent::DepsUnsatisfied,
        TicketEvent::Start,
        TicketEvent::AskHuman {
            reason: WaitingReason::Silent,
        },
        TicketEvent::InstructionDelivered,
        TicketEvent::AgentActive,
        TicketEvent::Answered,
        TicketEvent::Finished {
            summary: "s".to_owned(),
            open_threads: vec![],
            mode: crate::ReviewMode::Human,
            check: false,
            max_rounds: 3,
        },
        TicketEvent::Block { reason: manual() },
        TicketEvent::Unblock,
        TicketEvent::Return {
            comment: "c".to_owned(),
        },
        TicketEvent::Accept { comment: None },
        TicketEvent::MergeDone,
        TicketEvent::Released,
        TicketEvent::Reopen,
        TicketEvent::Pause,
        TicketEvent::Cancel {
            reason: "r".to_owned(),
        },
        TicketEvent::Approve { by: Actor::Human },
        TicketEvent::Reject {
            reason: "r".to_owned(),
            by: Actor::Human,
        },
    ]
}

#[test]
fn only_table_transitions_are_allowed() {
    use TicketStatus as S;
    let allowed: &[(S, &str)] = &[
        (S::Backlog, "deps_satisfied"),
        (S::Ready, "deps_unsatisfied"),
        (S::Ready, "start"),
        (S::Running, "ask_human"),
        (S::Waiting, "ask_human"),
        (S::Waiting, "instruction_delivered"),
        (S::Waiting, "agent_active"),
        (S::Waiting, "answered"),
        (S::Running, "finished"),
        (S::Running, "block"),
        (S::Waiting, "block"),
        (S::Blocked, "unblock"),
        (S::Review, "return"),
        (S::Review, "accept"),
        (S::Merged, "released"),
        (S::Merged, "reopen"),
        (S::Proposed, "approve"),
        (S::Proposed, "reject"),
    ];
    for status in TicketStatus::ALL {
        for event in sample_events() {
            let pausable = matches!(
                status,
                S::Backlog | S::Ready | S::Running | S::Waiting | S::Blocked
            );
            let cancellable =
                !matches!(status, S::Proposed | S::Merged | S::Released | S::Cancelled);
            let expected = allowed.contains(&(status, event.name()))
                || (event.name() == "pause" && pausable)
                || (event.name() == "cancel" && cancellable);
            let result = apply(&ticket_in(status), &event, NOW);
            assert_eq!(
                result.is_ok(),
                expected,
                "{status:?} + {}: {result:?}",
                event.name()
            );
        }
    }
}

// ---- property: no sequence of events breaks the invariants ----

fn any_event() -> impl Strategy<Value = TicketEvent> {
    let reason = prop_oneof![
        Just(manual()),
        Just(BlockedReason::Crashed {
            detail: "c".to_owned()
        }),
        Just(BlockedReason::Terminated {
            detail: "t".to_owned()
        }),
        Just(BlockedReason::RunnerLost),
        Just(BlockedReason::ShipFailed {
            detail: "s".to_owned()
        }),
        Just(BlockedReason::NoWorkingDir),
        Just(BlockedReason::MergeConflict {
            detail: "m".to_owned()
        }),
        Just(BlockedReason::BaseCheckoutDirty {
            path: "p".to_owned()
        }),
        Just(BlockedReason::ShipUncommitted {
            files: vec!["u".to_owned()]
        }),
        Just(BlockedReason::ShipEmpty),
        Just(BlockedReason::MergeFailed {
            detail: "f".to_owned()
        }),
        Just(BlockedReason::DependencyReopened {
            dependency: "DEMO-9".parse().expect("key")
        }),
        Just(BlockedReason::Manual {
            text: " ".to_owned()
        }),
    ];
    prop_oneof![
        Just(TicketEvent::DepsSatisfied),
        Just(TicketEvent::DepsUnsatisfied),
        Just(TicketEvent::Request),
        Just(TicketEvent::Start),
        Just(TicketEvent::ShipReady {
            branch: "b".to_owned()
        }),
        Just(TicketEvent::AskHuman {
            reason: WaitingReason::Silent
        }),
        Just(TicketEvent::InstructionDelivered),
        Just(TicketEvent::AgentActive),
        Just(TicketEvent::Answered),
        any::<u8>().prop_map(|n| TicketEvent::Finished {
            summary: "s".to_owned(),
            open_threads: vec![],
            mode: [
                crate::ReviewMode::Human,
                crate::ReviewMode::Agent,
                crate::ReviewMode::Both
            ][usize::from(n) % 3],
            check: n % 2 == 0,
            max_rounds: 3,
        }),
        any::<bool>().prop_map(|passed| TicketEvent::CheckDone {
            passed,
            output: "out".to_owned(),
            mode: crate::ReviewMode::Both,
            max_rounds: 1,
        }),
        Just(TicketEvent::CheckCannotRun {
            command: "cargo test".to_owned()
        }),
        Just(TicketEvent::CheckLost {
            detail: "d".to_owned()
        }),
        (any::<bool>(), "[a-z]{0,2}").prop_map(|(approve, comment)| TicketEvent::ReviewVerdict {
            verdict: if approve {
                crate::Verdict::Approve
            } else {
                crate::Verdict::RequestChanges
            },
            comment,
            items: vec![],
            max_rounds: 1,
        }),
        Just(TicketEvent::ReviewerFailed {
            detail: "d".to_owned()
        }),
        reason.prop_map(|reason| TicketEvent::Block { reason }),
        Just(TicketEvent::Unblock),
        Just(TicketEvent::Return {
            comment: "c".to_owned()
        }),
        Just(TicketEvent::Return {
            comment: String::new()
        }),
        Just(TicketEvent::Accept { comment: None }),
        Just(TicketEvent::MergeDone),
        Just(TicketEvent::Released),
        Just(TicketEvent::Reopen),
        Just(TicketEvent::Pause),
        Just(TicketEvent::Resume),
        Just(TicketEvent::Cancel {
            reason: "r".to_owned()
        }),
        Just(TicketEvent::Approve { by: Actor::Human }),
        Just(TicketEvent::Approve { by: Actor::Agent }),
        Just(TicketEvent::Reject {
            reason: "r".to_owned(),
            by: Actor::Human
        }),
    ]
}

proptest! {
    #[test]
    fn no_event_sequence_breaks_invariants(
        start in prop_oneof![Just(TicketStatus::Backlog), Just(TicketStatus::Proposed)],
        events in proptest::collection::vec(any_event(), 0..60),
    ) {
        let mut t = ticket_in(start);
        let mut now = T0;
        for event in &events {
            now = Timestamp(now.0 + 1);
            if let Ok(tr) = apply(&t, event, now) {
                    if let Err(e) = check_invariants(&tr.ticket) {
                        prop_assert!(false, "{} after {:?}", e, event);
                    }
                    prop_assert!(tr.ticket.status_since >= t.status_since);
                    if tr.ticket.status != t.status {
                        prop_assert_eq!(tr.ticket.status_since, now);
                    }
                    // Identity never changes.
                    prop_assert_eq!(tr.ticket.id, t.id);
                    prop_assert_eq!(&tr.ticket.key, &t.key);
                    t = tr.ticket;
            }
        }
    }
}

mod design;
mod merge;
mod no_checks;
mod out_of_order;
mod review;

fn base_behind(can_fast_forward: bool) -> TicketEvent {
    TicketEvent::Block {
        reason: BlockedReason::BaseBehind {
            base: "main".to_owned(),
            upstream: "origin/main".to_owned(),
            behind: 133,
            can_fast_forward,
        },
    }
}

#[test]
fn a_base_behind_blocks_the_start_and_offers_the_two_decisions() {
    let blocked = ok(TicketStatus::Running, base_behind(true));
    assert_eq!(blocked.effects, vec![Effect::StopRunner]);
    let names: Vec<_> = TicketAction::allowed(&blocked.ticket)
        .into_iter()
        .map(TicketAction::name)
        .collect();
    assert_eq!(
        names,
        [
            "pause",
            "unblock",
            "fast_forward_start",
            "start_anyway",
            "ask_design",
            "cancel"
        ]
    );

    let ff = apply(&blocked.ticket, &TicketEvent::FastForwardStart, NOW).expect("ff");
    assert_eq!(ff.ticket.status, TicketStatus::Running);
    assert_eq!(
        ff.effects,
        vec![
            Effect::PrepareShipWith {
                stale: StalePolicy::FastForward
            },
            Effect::EnsureRunner
        ]
    );
    let anyway = apply(&blocked.ticket, &TicketEvent::StartAnyway, NOW).expect("anyway");
    assert_eq!(
        anyway.effects[0],
        Effect::PrepareShipWith {
            stale: StalePolicy::Allow
        }
    );
    let again = apply(&blocked.ticket, &TicketEvent::Unblock, NOW).expect("unblock");
    assert_eq!(
        again.effects,
        vec![Effect::PrepareShip, Effect::EnsureRunner]
    );
}

#[test]
fn fast_forward_is_not_offered_when_it_is_not_possible_and_other_blocks_have_neither() {
    let blocked = ok(TicketStatus::Running, base_behind(false));
    assert!(apply(&blocked.ticket, &TicketEvent::FastForwardStart, NOW).is_err());
    assert!(apply(&blocked.ticket, &TicketEvent::StartAnyway, NOW).is_ok());
    let other = ticket_in(TicketStatus::Blocked);
    assert!(apply(&other, &TicketEvent::StartAnyway, NOW).is_err());
    assert!(apply(&other, &TicketEvent::FastForwardStart, NOW).is_err());
}
