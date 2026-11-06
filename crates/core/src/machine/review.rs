//! The review part of the transition table: who a card in Review waits for, how a verdict
//! (a person's `Accept` and `Return`, or the reviewer agent's `ReviewVerdict`) is recorded as a
//! round, and what each outcome asks of the host.
//!
//! The worker's runner is never replaced by the reviewer's: the reviewer runs next to it on its
//! own runner, so a `Return` goes back to a worker session that still has its context.

use super::checks::{non_empty, run_on_ship};
use super::{Effect, TransitionError};
use crate::{
    Actor, ReviewMode, ReviewTurn, StatusReason, Ticket, TicketStatus, Timestamp, Verdict,
};

/// The card was handed in: it waits for whoever `mode` says. A reviewer agent is started when
/// the agent's verdict comes first.
pub(super) fn enter(next: &mut Ticket, mode: ReviewMode, effects: &mut Vec<Effect>) {
    let turn = mode.first_turn();
    if matches!(turn, ReviewTurn::Agent { .. }) {
        effects.push(Effect::StartReviewer);
    }
    next.review.turn = Some(turn);
}

/// The card leaves Review: nobody is awaited any more.
pub(super) fn leave(next: &mut Ticket, effects: &mut Vec<Effect>) {
    if matches!(next.review.turn, Some(ReviewTurn::Agent { .. })) {
        effects.push(Effect::StopReviewer);
    }
    next.review.turn = None;
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

/// The reviewer agent gave its verdict. It is only heard while the card waits for the agent.
/// In agent mode the verdict moves the card; in both mode the card then waits for a person, who
/// sees the round.
pub(super) fn agent_decides(
    next: &mut Ticket,
    verdict: Verdict,
    comment: &str,
    now: Timestamp,
    effects: &mut Vec<Effect>,
) -> Result<Outcome, TransitionError> {
    let Some(ReviewTurn::Agent { decides }) = next.review.turn else {
        return Err(TransitionError::NotAllowed {
            event: "review_verdict",
            from: next.status,
        });
    };
    if verdict == Verdict::RequestChanges {
        non_empty(comment, "request_changes")?;
    }
    next.review.record(Actor::Agent, verdict, comment, now);
    if !decides {
        next.review.turn = Some(ReviewTurn::Person);
        return Ok((TicketStatus::Review, None));
    }
    next.review.turn = None;
    Ok(match verdict {
        Verdict::Approve => {
            start_merge(next, effects);
            (TicketStatus::Review, None)
        }
        Verdict::RequestChanges => {
            effects.push(Effect::RecordRemarks {
                text: comment.trim().to_owned(),
            });
            run_on_ship(effects);
            (TicketStatus::Running, None)
        }
    })
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
    effects.push(Effect::StopReviewer);
    effects.push(Effect::RecordReport {
        summary: format!("The reviewer gave no verdict ({detail}); it is for a person to decide."),
    });
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

/// A card whose merge is running takes no verdict and no return.
pub(super) fn not_merging(
    ticket: &Ticket,
    event: &super::TicketEvent,
) -> Result<(), TransitionError> {
    if ticket.review.turn == Some(ReviewTurn::Merging) {
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
