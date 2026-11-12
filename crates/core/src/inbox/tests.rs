//! The inbox rules, one test per kind of item.

use super::*;
use crate::{
    Actor, ArtifactFormat, BlockedReason, MessageKind, ReviewTurn, StatusReason, ThreadId,
    ThreadKind, ThreadMessageId, ThreadState, ThreadVersionId, TicketId, TicketStatus,
    WaitingReason, Wildcard,
};

mod hand_in_items;
mod leftover_items;
mod pin_items;
mod question_items;

pub(super) fn ticket(n: u32, status: TicketStatus, since: i64) -> Ticket {
    let reason = match status {
        TicketStatus::Proposed => Some(StatusReason::Proposed),
        _ => None,
    };
    Ticket {
        id: TicketId::new(),
        chain_id: ChainId::new(),
        key: format!("DEMO-{n}").parse().expect("key"),
        title: format!("card {n}"),
        prompt: String::new(),
        acceptance: Vec::new(),
        status,
        status_reason: reason,
        status_since: Timestamp(since),
        paused: false,
        queued: false,
        profile: None,
        review_mode: None,
        reviewer: None,
        model: None,
        review: crate::Review::default(),
        pos: None,
        size: None,
        branch: None,
        needs_secrets: false,
        design_first: false,
        designs: Vec::new(),
        tokens_used: 0,
        wildcard: None,
        done_out_of_order: None,
        pr: None,
    }
}

fn waiting(mut t: Ticket, reason: WaitingReason) -> Ticket {
    t.status = TicketStatus::Waiting;
    t.status_reason = Some(StatusReason::Waiting(reason));
    t
}

fn blocked(mut t: Ticket, reason: BlockedReason) -> Ticket {
    t.status = TicketStatus::Blocked;
    t.status_reason = Some(StatusReason::Blocked {
        reason,
        resume_to: TicketStatus::Running,
        waiting: None,
    });
    t
}

fn items(tickets: Vec<Ticket>) -> Vec<InboxItem> {
    compute(&InboxData {
        tickets,
        ..InboxData::default()
    })
}

#[test]
fn a_wildcard_card_awaiting_approval_is_a_proposal() {
    let mut t = ticket(1, TicketStatus::Proposed, 10);
    t.wildcard = Some(Wildcard {
        from: None,
        proposed_by: Actor::Agent,
        after: vec![],
        before: vec![],
        approved: false,
    });
    let got = items(vec![t.clone()]);
    assert_eq!(got.len(), 1);
    let i = &got[0];
    assert_eq!(
        (i.kind, &i.ticket, i.since),
        (InboxKind::Proposal, &Some(t.key.clone()), Timestamp(10))
    );
    assert_eq!((i.title.as_str(), i.chain), ("card 1", Some(t.chain_id)));
}

#[test]
fn a_question_carries_its_text_and_a_permission_prompt_counts_too() {
    let q = waiting(
        ticket(1, TicketStatus::Running, 5),
        WaitingReason::Question {
            text: "which framework?".to_owned(),
        },
    );
    let p = waiting(
        ticket(2, TicketStatus::Running, 6),
        WaitingReason::Permission {
            text: "run rm -rf build?".to_owned(),
            detail: None,
        },
    );
    let silent = waiting(ticket(3, TicketStatus::Running, 7), WaitingReason::Silent);
    let got = items(vec![q, p, silent]);
    let shown: Vec<_> = got.iter().map(|i| (i.kind, i.text.as_str())).collect();
    assert_eq!(
        shown,
        [
            (InboxKind::Question, "run rm -rf build?"),
            (InboxKind::Question, "which framework?")
        ]
    );
}

#[test]
fn a_blocked_card_carries_its_reason() {
    let t = blocked(
        ticket(1, TicketStatus::Running, 5),
        BlockedReason::Manual {
            text: "waiting for the API key".to_owned(),
        },
    );
    let got = items(vec![t]);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].kind, InboxKind::Blocked);
    assert_eq!(got[0].text, "manual: waiting for the API key");
}

fn reviewed_by(turn: Option<ReviewTurn>) -> Ticket {
    let mut t = ticket(1, TicketStatus::Review, 5);
    t.review.turn = turn;
    t
}

#[test]
fn a_card_in_review_waits_for_the_person_only_when_it_is_the_persons_turn() {
    let person = items(vec![reviewed_by(Some(ReviewTurn::Person))]);
    assert_eq!(person.len(), 1);
    assert_eq!(person[0].kind, InboxKind::Review);
    // Agent mode: the reviewer decides; both mode: the person comes after the verdict.
    for decides in [true, false] {
        let turn = Some(ReviewTurn::Agent { decides });
        assert_eq!(items(vec![reviewed_by(turn)]), [], "decides={decides}");
    }
}

#[test]
fn a_merging_card_is_nobodys_turn_and_is_not_listed() {
    assert_eq!(items(vec![reviewed_by(Some(ReviewTurn::Merging))]), []);
}

#[test]
fn a_card_whose_merge_failed_is_a_blocked_item_with_the_reason() {
    let reasons = [
        (
            BlockedReason::MergeConflict {
                detail: "conflicting files: a.rs".to_owned(),
            },
            "merge_conflict: conflicting files: a.rs",
        ),
        (
            BlockedReason::BaseCheckoutDirty {
                path: "/repo".to_owned(),
            },
            "base_checkout_dirty: /repo has local changes; commit or stash them, then unblock",
        ),
        (
            BlockedReason::MergeFailed {
                detail: "too old".to_owned(),
            },
            "merge_failed: too old",
        ),
        (
            BlockedReason::ShipUncommitted {
                files: vec!["a.rs".to_owned()],
            },
            "ship_uncommitted: the ship has changes that are not committed (a.rs); commit them on the ship, then unblock",
        ),
        (
            BlockedReason::ShipEmpty,
            "ship_empty: the ship's branch has no commit of its own, so there is nothing to merge; the work has to be committed on the ship, then unblock",
        ),
    ];
    for (reason, text) in reasons {
        let mut t = ticket(1, TicketStatus::Review, 5);
        t = blocked(t, reason);
        let got = items(vec![t]);
        assert_eq!(got.len(), 1, "{text}");
        assert_eq!(
            (got[0].kind, got[0].text.as_str()),
            (InboxKind::Blocked, text)
        );
    }
}

#[test]
fn an_item_carries_the_actions_the_machine_allows_the_person() {
    use crate::machine::TicketAction;
    let blocked = blocked(
        ticket(1, TicketStatus::Running, 5),
        BlockedReason::RunnerLost,
    );
    let got = items(vec![blocked.clone()]);
    assert_eq!(got[0].allowed_actions, TicketAction::allowed(&blocked));
    assert!(got[0].allowed_actions.contains(&TicketAction::Unblock));
    let proposed = ticket(2, TicketStatus::Proposed, 6);
    let got = items(vec![proposed.clone()]);
    assert_eq!(got[0].allowed_actions, TicketAction::allowed(&proposed));
    assert!(got[0].allowed_actions.contains(&TicketAction::Approve));
    assert!(got[0].allowed_actions.contains(&TicketAction::Reject));
}

#[test]
fn a_card_in_other_states_is_not_listed() {
    let mut all = vec![reviewed_by(Some(ReviewTurn::Person))];
    for (n, s) in [
        TicketStatus::Backlog,
        TicketStatus::Ready,
        TicketStatus::Running,
        TicketStatus::Merged,
        TicketStatus::Released,
        TicketStatus::Cancelled,
    ]
    .into_iter()
    .enumerate()
    {
        all.push(ticket(n as u32 + 2, s, 5));
    }
    let got = items(all);
    assert_eq!(got.len(), 1);
    assert_eq!(
        (
            got[0].kind,
            got[0].ticket.clone().expect("card").to_string()
        ),
        (InboxKind::Review, "DEMO-1".to_owned())
    );
}

fn thread_facts(t: &Ticket, state: ThreadState, versions: &[i64], changes: &[i64]) -> ThreadFacts {
    let id = ThreadId::new();
    ThreadFacts {
        ticket: t.key.clone(),
        thread: Thread {
            id,
            ticket_id: t.id,
            number: 2,
            kind: ThreadKind::Design,
            title: "List states".to_owned(),
            state,
            opened_by: Actor::Agent,
            created_at: Timestamp(1),
            approved_version: None,
            approved_at: None,
        },
        versions: versions
            .iter()
            .enumerate()
            .map(|(n, at)| ThreadVersion {
                id: ThreadVersionId::new(),
                thread_id: id,
                number: n as u32 + 1,
                author: Actor::Agent,
                format: ArtifactFormat::Mermaid,
                body: "graph TD; A-->B".to_owned(),
                created_at: Timestamp(*at),
            })
            .collect(),
        messages: changes
            .iter()
            .map(|at| ThreadMessage {
                id: ThreadMessageId::new(),
                thread_id: id,
                author: Actor::Human,
                kind: MessageKind::ChangesRequested,
                text: "larger".to_owned(),
                created_at: Timestamp(*at),
            })
            .collect(),
    }
}

fn with_threads(t: &Ticket, threads: Vec<ThreadFacts>) -> Vec<InboxItem> {
    compute(&InboxData {
        tickets: vec![t.clone()],
        threads,
        ..InboxData::default()
    })
}

#[test]
fn an_open_thread_with_a_version_the_person_has_not_answered_is_listed() {
    let t = ticket(1, TicketStatus::Running, 1);
    let open = thread_facts(&t, ThreadState::Open, &[10, 30], &[20]);
    let got = with_threads(&t, vec![open]);
    assert_eq!(got.len(), 1);
    let i = &got[0];
    assert_eq!(
        (i.kind, i.thread, i.text.as_str(), i.since),
        (InboxKind::Thread, Some(2), "List states", Timestamp(30))
    );
}

#[test]
fn a_thread_waiting_for_the_agent_or_approved_or_without_a_version_is_not_listed() {
    let t = ticket(1, TicketStatus::Running, 1);
    let answered = thread_facts(&t, ThreadState::Open, &[10], &[20]);
    let approved = thread_facts(&t, ThreadState::Approved, &[10], &[]);
    let empty = thread_facts(&t, ThreadState::Open, &[], &[]);
    assert_eq!(with_threads(&t, vec![answered, approved, empty]), []);
}

#[test]
fn items_come_newest_first_and_ties_follow_the_key() {
    let a = blocked(
        ticket(1, TicketStatus::Running, 10),
        BlockedReason::RunnerLost,
    );
    let b = ticket(2, TicketStatus::Review, 30);
    let c = ticket(3, TicketStatus::Review, 30);
    let keys: Vec<_> = items(vec![a, b, c])
        .iter()
        .map(|i| i.ticket.clone().expect("card").to_string())
        .collect();
    assert_eq!(keys, ["DEMO-2", "DEMO-3", "DEMO-1"]);
}

#[test]
fn kinds_are_ordered_for_grouping() {
    use InboxKind::*;
    let mut kinds = vec![Leftovers, Thread, Review, Blocked, Question, Proposal];
    kinds.sort();
    assert_eq!(
        kinds,
        [Proposal, Question, Blocked, Review, Thread, Leftovers]
    );
}

#[test]
fn a_blocked_card_with_a_pull_request_carries_its_link_and_other_cards_do_not() {
    let pr = crate::PullRequest::seen(7, "https://github.com/o/r/pull/7".to_owned(), Vec::new());
    let mut failed = blocked(
        ticket(1, TicketStatus::Review, 5),
        BlockedReason::CiFailed {
            url: pr.url.clone(),
            checks: vec!["verify".to_owned()],
        },
    );
    failed.pr = Some(pr.clone());
    let mut running = ticket(2, TicketStatus::Review, 6);
    running.pr = Some(pr.clone());
    running.review.turn = Some(ReviewTurn::Person);
    let got = items(vec![failed, running]);
    let link = |key: &str| {
        got.iter()
            .find(|i| i.ticket.as_ref().is_some_and(|t| t.to_string() == key))
            .map(|i| i.link.clone())
    };
    assert_eq!(link("DEMO-1"), Some(Some(pr.url)));
    assert_eq!(link("DEMO-2"), Some(None));
    let text = got
        .iter()
        .find(|i| i.kind == InboxKind::Blocked)
        .map(|i| i.text.clone());
    assert!(text.is_some_and(|t| t.starts_with("ci_failed: verify failed on ")));
}
