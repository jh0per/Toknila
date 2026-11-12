//! The review part of the transition table: who a card in Review waits for, how a verdict
//! (a person's `Accept` and `Return`, or the reviewer agent's `ReviewVerdict`) is recorded as a
//! round, and what each outcome asks of the host.
//!
//! The worker's runner is never replaced by the reviewer's: the reviewer runs next to it on its
//! own runner, so a `Return` goes back to a worker session that still has its context.

use super::checks::{non_empty, run_on_ship};
use super::{Effect, TransitionError};
use crate::{
    Actor, ItemVerdict, ReviewMode, ReviewNote, ReviewTurn, StatusReason, Ticket, TicketStatus,
    Timestamp, Verdict, check_items, effective_verdict, render_items, request_is_unfounded,
};

/// What the reviewer is told when it ends its turn without a verdict.
pub const NUDGE_MESSAGE: &str = "Record your verdict with review_verdict; if you could not run something, say so in the verdict.";

/// The most lines of the reviewer's screen a note keeps.
const NOTE_LINES: usize = crate::question::SCREEN_LINES;

/// The card was handed in: it waits for whoever `mode` says. A reviewer agent is started when
/// the agent's verdict comes first.
pub(super) fn enter(next: &mut Ticket, mode: ReviewMode, effects: &mut Vec<Effect>) {
    let turn = mode.first_turn();
    if matches!(turn, ReviewTurn::Agent { .. }) {
        effects.push(Effect::StartReviewer);
    }
    next.review.turn = Some(turn);
    next.review.nudged = false;
    next.review.note = None;
}

/// The card leaves Review: nobody is awaited any more.
pub(super) fn leave(next: &mut Ticket, effects: &mut Vec<Effect>) {
    if matches!(next.review.turn, Some(ReviewTurn::Agent { .. })) {
        effects.push(Effect::StopReviewer);
    }
    next.review.turn = None;
    next.review.check = None;
    next.review.nudged = false;
    next.review.note = None;
}

/// A person decides (accept or return), whatever the reviewer agent is doing: it is stopped
/// when it was still looking, and the decision is a round of its own.
pub(super) fn person_decides(
    next: &mut Ticket,
    verdict: Verdict,
    comment: &str,
    now: Timestamp,
    effects: &mut Vec<Effect>,
) {
    leave(next, effects);
    next.review.record(Actor::Human, verdict, comment, now);
}

/// Where a card goes after a verdict, as a status and its reason.
pub(super) type Outcome = (TicketStatus, Option<StatusReason>);

/// The check ran. A pass goes on to whoever the review mode says (the reviewer agent, or the
/// person, who then sees a card that already passes). A failure sends the card back to the
/// worker with the output, a round of the loop, until the project's round limit is used: then
/// the card waits for the person with every round in its history.
pub(super) fn check_done(
    next: &mut Ticket,
    (passed, output): (bool, &str),
    (mode, max_rounds): (ReviewMode, u32),
    now: Timestamp,
    effects: &mut Vec<Effect>,
) -> Result<Outcome, TransitionError> {
    if next.review.turn != Some(ReviewTurn::Check) {
        return Err(TransitionError::NotAllowed {
            event: "check_done",
            from: next.status,
        });
    }
    let command = match &next.review.check {
        Some(crate::CheckPhase::Running { command, .. }) => command.clone(),
        _ => String::new(),
    };
    next.review.check = None;
    next.review.last_check = Some(crate::CheckOutcome {
        command,
        passed,
        output: output.to_owned(),
        at: now,
    });
    if passed {
        next.review
            .record(Actor::System, Verdict::Approve, "The check passed.", now);
        enter(next, mode, effects);
        return Ok((TicketStatus::Review, None));
    }
    next.review
        .record(Actor::System, Verdict::RequestChanges, output, now);
    send_back(next, max_rounds, "The check failed", output, effects, true)
}

/// A round of the loop asked for changes: the card goes back to the worker with `text`, unless
/// `max_rounds` returns were already used since a person last decided; then it waits for the
/// person. `by_check` says whether the text is the check's (and so not the reviewer's remarks).
fn send_back(
    next: &mut Ticket,
    max_rounds: u32,
    what: &str,
    text: &str,
    effects: &mut Vec<Effect>,
    by_check: bool,
) -> Result<Outcome, TransitionError> {
    let used = next.review.auto_returns();
    next.review.nudged = false;
    if used > max_rounds {
        next.review.turn = Some(ReviewTurn::Person);
        effects.push(Effect::RecordNotice {
            summary: format!(
                "{what} again, and the {max_rounds} rounds of the loop are used; it is for a person to decide."
            ),
        });
        return Ok((TicketStatus::Review, None));
    }
    next.review.turn = None;
    effects.push(if by_check {
        Effect::RecordCheckOutput {
            text: format!(
                "{what} (round {used} of {max_rounds}). Fix what it reports, then finish again.\n\n{text}"
            ),
        }
    } else {
        Effect::RecordRemarks {
            text: text.to_owned(),
        }
    });
    run_on_ship(effects);
    Ok((TicketStatus::Running, None))
}

/// What the reviewer agent said, as the machine takes it.
pub(super) struct AgentVerdict<'a> {
    pub verdict: Verdict,
    pub comment: &'a str,
    pub items: &'a [ItemVerdict],
    pub max_rounds: u32,
}

/// The reviewer agent gave its verdict, heard while the card waits for the agent; it must judge
/// every acceptance item. An approval that decides starts the merge (in both mode the person
/// follows); a request for changes is a round that returns the card to the worker until the
/// round limit is used, unless it is unfounded (nothing run or read fails): then no round.
pub(super) fn agent_decides(
    next: &mut Ticket,
    said: AgentVerdict<'_>,
    now: Timestamp,
    effects: &mut Vec<Effect>,
) -> Result<Outcome, TransitionError> {
    let Some(ReviewTurn::Agent { decides }) = next.review.turn else {
        return Err(TransitionError::NotAllowed {
            event: "review_verdict",
            from: next.status,
        });
    };
    let verdict = effective_verdict(said.verdict, said.items);
    check_items(&next.acceptance, said.items, verdict)?;
    let text = render_items(&next.acceptance, said.items, said.comment);
    if verdict == Verdict::RequestChanges {
        non_empty(&text, "request_changes")?;
    }
    if verdict == Verdict::RequestChanges && request_is_unfounded(said.items, said.comment) {
        return super::unfounded::wait_for_person(next, &text, effects);
    }
    next.review.record(Actor::Agent, verdict, &text, now);
    // The reviewer is done: its process ends and its slot is free. The worker's runner stays.
    effects.push(Effect::StopReviewer);
    match verdict {
        Verdict::Approve if !decides => {
            next.review.turn = Some(ReviewTurn::Person);
            Ok((TicketStatus::Review, None))
        }
        Verdict::Approve => {
            next.review.turn = None;
            start_merge(next, effects);
            Ok((TicketStatus::Review, None))
        }
        Verdict::RequestChanges => send_back(
            next,
            said.max_rounds,
            "The reviewer asked for changes",
            &text,
            effects,
            false,
        ),
    }
}

/// The reviewer could not give a verdict: the card waits for a person instead.
pub(super) fn agent_failed(
    next: &mut Ticket,
    detail: &str,
    effects: &mut Vec<Effect>,
) -> Result<Outcome, TransitionError> {
    if !matches!(next.review.turn, Some(ReviewTurn::Agent { .. })) {
        return Err(TransitionError::NotAllowed {
            event: "reviewer_failed",
            from: next.status,
        });
    }
    next.review.turn = Some(ReviewTurn::Person);
    next.review.note = Some(ReviewNote::NoVerdict { lines: Vec::new() });
    effects.push(Effect::StopReviewer);
    effects.push(Effect::RecordNotice {
        summary: format!("The reviewer gave no verdict ({detail}); it is for a person to decide."),
    });
    Ok((TicketStatus::Review, None))
}

/// The reviewer ended its turn while the card still waits for its verdict. The first time it is
/// nudged (once, by the host); the second time the card goes to a person with a note that the
/// reviewer gave no verdict and the last lines of its screen.
pub(super) fn agent_stopped(
    next: &mut Ticket,
    screen: &[String],
    effects: &mut Vec<Effect>,
) -> Result<Outcome, TransitionError> {
    if !matches!(next.review.turn, Some(ReviewTurn::Agent { .. })) {
        return Err(TransitionError::NotAllowed {
            event: "reviewer_stopped",
            from: next.status,
        });
    }
    if !next.review.nudged {
        next.review.nudged = true;
        effects.push(Effect::NudgeReviewer {
            message: NUDGE_MESSAGE.to_owned(),
        });
        return Ok((TicketStatus::Review, None));
    }
    let from = screen.len().saturating_sub(NOTE_LINES);
    next.review.turn = Some(ReviewTurn::Person);
    next.review.note = Some(ReviewNote::NoVerdict {
        lines: screen[from..].to_vec(),
    });
    effects.push(Effect::StopReviewer);
    Ok((TicketStatus::Review, None))
}

/// A person asks for the reviewer again after it gave no verdict.
pub(super) fn rerun(
    next: &mut Ticket,
    effects: &mut Vec<Effect>,
) -> Result<Outcome, TransitionError> {
    if next.review.turn != Some(ReviewTurn::Person)
        || !matches!(next.review.note, Some(ReviewNote::NoVerdict { .. }))
    {
        return Err(TransitionError::NotAllowed {
            event: "run_reviewer",
            from: next.status,
        });
    }
    next.review.turn = Some(ReviewTurn::Agent { decides: false });
    next.review.nudged = false;
    next.review.note = None;
    effects.push(Effect::StartReviewer);
    Ok((TicketStatus::Review, None))
}

/// The work was accepted: the host merges the ship's branch into the base branch, and the card
/// stays in Review (merging) until the outcome comes back. The worker's runner stays alive, so
/// whoever has to fix a conflict can do it on the ship.
pub(super) fn start_merge(next: &mut Ticket, effects: &mut Vec<Effect>) {
    next.review.turn = Some(ReviewTurn::Merging);
    // A merge that starts opens its pull request (again): the one seen before is stale, and
    // only a card with a pull request is watched, so the ticker cannot judge old checks.
    next.pr = None;
    effects.push(Effect::Merge);
}

/// The merge is waiting on a pull request with no check reported and a person was asked.
pub(super) fn asked_about_no_checks(ticket: &Ticket) -> bool {
    ticket.review.turn == Some(ReviewTurn::Merging)
        && ticket.review.note == Some(crate::ReviewNote::NoChecks)
        && ticket.pr.is_some()
}

/// Whether `reason` is a person's block as the third answer to the no-checks question.
pub(super) fn blocks_unasked(ticket: &Ticket, reason: &crate::BlockedReason) -> bool {
    asked_about_no_checks(ticket) && matches!(reason, crate::BlockedReason::Manual { .. })
}

/// The host looked at the pull request. The machine keeps the timers the host does not know:
/// since when no check was reported, and a person's "merge now". When no check has been reported
/// for `ask_after_secs` the card asks a person (once; a later look changes nothing).
pub(super) fn pull_request_seen(
    next: &mut Ticket,
    mut pr: crate::PullRequest,
    ask_after_secs: u64,
    now: Timestamp,
) -> Option<()> {
    if next.review.turn != Some(ReviewTurn::Merging) {
        return None;
    }
    let before = next.pr.take().filter(|p| p.number == pr.number);
    pr.merge_unchecked = before.as_ref().is_some_and(|p| p.merge_unchecked);
    if pr.checks.is_empty() {
        let since = before.and_then(|p| p.no_checks_since).unwrap_or(now);
        pr.no_checks_since = Some(since);
        let waited = now.millis_since(since) / 1000;
        if waited >= ask_after_secs && !pr.merge_unchecked && next.review.note.is_none() {
            next.review.note = Some(crate::ReviewNote::NoChecks);
        }
    } else if next.review.note == Some(crate::ReviewNote::NoChecks) {
        next.review.note = None;
    }
    next.pr = Some(pr);
    Some(())
}

/// A person answered the no-checks question with "merge now" (the host merges although no check
/// was reported) or "keep waiting" (the timeout starts again). `None` when nobody asked.
pub(super) fn no_checks_answer(
    next: &mut Ticket,
    event: &super::TicketEvent,
    now: Timestamp,
    effects: &mut Vec<Effect>,
) -> Option<()> {
    if !asked_about_no_checks(next) {
        return None;
    }
    next.review.note = None;
    let pr = next.pr.as_mut()?;
    if matches!(event, super::TicketEvent::MergeUnchecked) {
        pr.merge_unchecked = true;
        effects.push(Effect::Merge);
    } else {
        pr.no_checks_since = Some(now);
    }
    Some(())
}

/// A card whose merge is running or whose check is running takes no verdict and no return.
pub(super) fn not_merging(
    ticket: &Ticket,
    event: &super::TicketEvent,
) -> Result<(), TransitionError> {
    if ticket.review.turn == Some(ReviewTurn::Check) {
        Err(TransitionError::BeingChecked {
            event: event.name(),
        })
    } else if ticket.review.turn == Some(ReviewTurn::Merging) {
        Err(TransitionError::NotAllowed {
            event: event.name(),
            from: ticket.status,
        })
    } else {
        Ok(())
    }
}

/// The merge landed: what that does to the host and the graph.
pub(super) fn merged(effects: &mut Vec<Effect>) {
    effects.push(Effect::RemoveShip);
    effects.push(Effect::StopRunner);
    effects.push(Effect::DependencySatisfied);
}
