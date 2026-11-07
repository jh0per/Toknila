//! The ticket state machine. Every status change goes through [`apply`]; an event that the
//! transition table does not allow from the current status is a typed [`TransitionError`].
//!
//! | From | To | Event |
//! |---|---|---|
//! | Backlog | Ready | `DepsSatisfied` (not paused) |
//! | Ready | Backlog | `DepsUnsatisfied` |
//! | Ready | Ready (queued) | `Request` (a person's start; the schedule turns it into `Start` when a slot is free) |
//! | Ready | Running | `Start` (the schedule decides; see `schedule`) |
//! | Running | Waiting | `AskHuman` |
//! | Waiting | Running | `InstructionDelivered`, `AgentActive`, `Answered` (a person answered and it reached the agent) |
//! | Running | Review | `Finished` |
//! | Running, Waiting | Blocked | `Block` (Review too, for a merge conflict) |
//! | Blocked | previous status | `Unblock` |
//! | Review | Running | `Return` (a person; the worker's runner is kept if alive) |
//! | Review | Review (merging) | `Accept`; an approving `ReviewVerdict` that decides does the same |
//! | Review (merging) | Merged | `MergeDone` (removes the ship's worktree, keeps its branch) |
//! | Review (merging) | Blocked | `Block` with a merge reason (`merge_conflict`, `ci_failed`, `pr_closed`, ...); `Unblock` merges again |
//! | Review (merging) | same | `PrUpdated` (pull-request mode: the card's pull request and its checks) |
//! | Review | Running / same | `ReviewVerdict` (the reviewer agent; see [`review`]) |
//! | Review | same | `ReviewerFailed` (the card then waits for a person) |
//! | Review | same | `ReviewerStopped` (the reviewer ended a turn without a verdict: nudged once, then the card waits for a person with a note) |
//! | Review | same | `RunReviewer` (a person asks for the reviewer again after it gave no verdict) |
//! | Backlog, Ready | Merged | `DoneOutOfOrder` (import only: work finished behind open cards) |
//! | Merged | Released | `Released` |
//! | Merged | Review | `Reopen` |
//! | Backlog, Ready, Running, Waiting, Blocked | Backlog (paused) | `Pause` |
//! | Running, Waiting | same | `ShipReady` (the ticket learns its branch) |
//! | Backlog (paused) | Backlog | `Resume` |
//! | any but Proposed, Merged, Released, Cancelled | Cancelled | `Cancel` (the ship stays) |
//! | Proposed | Backlog | `Approve` (a person only; the proposed edges follow) |
//! | Proposed | Cancelled | `Reject` (a person only, with a reason) |
//!
//! Pause is a flag, not a status: a paused ticket sits in Backlog. Graph-wide consequences
//! (dependents becoming Ready, Blocked or Backlog) come back as [`Effect`]s for the chain graph.

use crate::{
    Actor, OutOfOrder, ReviewTurn, StatusReason, Ticket, TicketKey, TicketStatus, Timestamp,
    Verdict, WaitingReason, thread::OpenThread,
};

mod action;
mod event;
pub use action::{TicketAction, UnknownAction};
pub use event::{Effect, TicketEvent};

/// The ticket after the event, and what must happen because of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transition {
    pub ticket: Ticket,
    pub effects: Vec<Effect>,
}

/// Why an event was refused. The ticket is unchanged.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransitionError {
    #[error("cannot {event} a ticket in {from:?}")]
    NotAllowed {
        event: &'static str,
        from: TicketStatus,
    },
    #[error("ticket is paused; resume it first")]
    Paused,
    #[error("ticket is already started and waits for a free slot")]
    AlreadyQueued,
    #[error("ticket is not paused")]
    NotPaused,
    #[error("{0} needs a text")]
    EmptyText(&'static str),
    #[error("cannot hand in {0}: {open}", open = crate::thread::list(.1))]
    OpenThreads(TicketKey, Vec<OpenThread>),
    #[error("a card is done out of order only when a card before it is still open")]
    NothingBehind,
    #[error("only a person can {event} a card; this came from {actor:?}")]
    PersonOnly { event: &'static str, actor: Actor },
}

/// Applies `event` to `ticket` at time `now`. Pure: same input, same output.
pub fn apply(
    ticket: &Ticket,
    event: &TicketEvent,
    now: Timestamp,
) -> Result<Transition, TransitionError> {
    use TicketStatus as S;

    let from = ticket.status;
    let refuse = || TransitionError::NotAllowed {
        event: event.name(),
        from,
    };
    let mut next = ticket.clone();
    let mut effects = Vec::new();

    let (status, reason) = match (from, event) {
        (S::Backlog, TicketEvent::DepsSatisfied) => {
            if ticket.paused {
                return Err(TransitionError::Paused);
            }
            (S::Ready, None)
        }
        (S::Ready, TicketEvent::DepsUnsatisfied) => (S::Backlog, None),
        (S::Ready, TicketEvent::Request) => {
            if ticket.queued {
                return Err(TransitionError::AlreadyQueued);
            }
            next.queued = true;
            (S::Ready, None)
        }
        (S::Ready, TicketEvent::Start) => {
            run_on_ship(&mut effects);
            (S::Running, None)
        }
        (S::Running, TicketEvent::AskHuman { reason }) => {
            (S::Waiting, Some(StatusReason::Waiting(reason.clone())))
        }
        (
            S::Waiting,
            TicketEvent::InstructionDelivered | TicketEvent::AgentActive | TicketEvent::Answered,
        ) => (S::Running, None),
        (
            S::Running,
            TicketEvent::Finished {
                summary,
                open_threads,
                mode,
            },
        ) => {
            crate::thread::require_none_open(&ticket.key, open_threads)?;
            effects.push(Effect::RecordReport {
                summary: summary.clone(),
            });
            review::enter(&mut next, *mode, &mut effects);
            (S::Review, None)
        }
        (S::Running | S::Waiting, TicketEvent::Block { reason }) => {
            block(reason, ticket, &mut effects)?
        }
        (S::Review, TicketEvent::Block { reason }) if reason.is_merge() => {
            review::leave(&mut next, &mut effects);
            block(reason, ticket, &mut effects)?
        }
        (S::Blocked, TicketEvent::Unblock) => {
            let (resume_to, waiting) = match &ticket.status_reason {
                Some(StatusReason::Blocked {
                    resume_to, waiting, ..
                }) => (*resume_to, waiting.clone()),
                // Invariant broken upstream; resume to the safest active state.
                _ => (S::Running, None),
            };
            match (resume_to, waiting) {
                (S::Waiting, Some(w)) => (S::Waiting, Some(StatusReason::Waiting(w))),
                (S::Waiting, None) => (
                    S::Waiting,
                    Some(StatusReason::Waiting(WaitingReason::Silent)),
                ),
                (S::Running, _) => {
                    run_on_ship(&mut effects);
                    (S::Running, None)
                }
                (S::Review, _) => {
                    // A merge that failed is tried again; the person or the agent has fixed
                    // what stopped it on the ship.
                    if matches!(&ticket.status_reason, Some(StatusReason::Blocked { reason, .. }) if reason.is_merge())
                    {
                        review::start_merge(&mut next, &mut effects);
                    } else {
                        next.review.turn = Some(ReviewTurn::Person);
                    }
                    (S::Review, None)
                }
                (other, _) => (other, None),
            }
        }
        (S::Review, TicketEvent::Return { comment }) => {
            review::not_merging(&next, event)?;
            non_empty(comment, "return")?;
            review::person_decides(
                &mut next,
                Verdict::RequestChanges,
                comment,
                now,
                &mut effects,
            );
            effects.push(Effect::RecordInstruction {
                text: comment.clone(),
            });
            run_on_ship(&mut effects);
            (S::Running, None)
        }
        (S::Review, TicketEvent::Accept { comment }) => {
            review::not_merging(&next, event)?;
            let remarks = comment.as_deref().unwrap_or_default();
            review::person_decides(&mut next, Verdict::Approve, remarks, now, &mut effects);
            review::start_merge(&mut next, &mut effects);
            (S::Review, None)
        }
        (S::Review, TicketEvent::MergeDone) => {
            if next.review.turn != Some(ReviewTurn::Merging) {
                return Err(refuse());
            }
            next.review.turn = None;
            review::merged(&mut effects);
            (S::Merged, None)
        }
        (S::Review, TicketEvent::PrUpdated { pr }) => {
            if next.review.turn != Some(ReviewTurn::Merging) {
                return Err(refuse());
            }
            next.pr = Some(pr.clone());
            (S::Review, None)
        }
        (S::Review, TicketEvent::ReviewVerdict { verdict, comment }) => {
            review::agent_decides(&mut next, *verdict, comment, now, &mut effects)?
        }
        (S::Review, TicketEvent::ReviewerFailed { detail }) => {
            review::agent_failed(&mut next, detail, &mut effects)?
        }
        (S::Review, TicketEvent::ReviewerStopped { screen }) => {
            review::agent_stopped(&mut next, screen, &mut effects)?
        }
        (S::Review, TicketEvent::RunReviewer) => review::rerun(&mut next, &mut effects)?,
        (S::Backlog | S::Ready, TicketEvent::DoneOutOfOrder { behind }) => {
            if behind.is_empty() {
                return Err(TransitionError::NothingBehind);
            }
            next.paused = false;
            next.done_out_of_order = Some(OutOfOrder::new(behind.clone()));
            effects.push(Effect::DependencySatisfied);
            (S::Merged, None)
        }
        (S::Merged, TicketEvent::Released) => (S::Released, None),
        (S::Merged, TicketEvent::Reopen) => {
            effects.push(Effect::DependencyReopened);
            next.review.turn = Some(ReviewTurn::Person);
            (S::Review, None)
        }
        (S::Backlog | S::Ready | S::Running | S::Waiting | S::Blocked, TicketEvent::Pause) => {
            if ticket.paused {
                return Err(refuse());
            }
            if matches!(from, S::Running | S::Waiting | S::Blocked) {
                effects.push(Effect::StopRunner);
            }
            next.paused = true;
            (S::Backlog, None)
        }
        (S::Running | S::Waiting, TicketEvent::ShipReady { branch }) => {
            next.branch = Some(branch.clone());
            (from, ticket.status_reason.clone())
        }
        (_, TicketEvent::Resume) => {
            if !ticket.paused {
                return Err(TransitionError::NotPaused);
            }
            next.paused = false;
            effects.push(Effect::RecheckReadiness);
            (from, ticket.status_reason.clone())
        }
        (s, TicketEvent::Cancel { reason }) if !s.is_done() && s != S::Proposed => {
            non_empty(reason, "cancel")?;
            if matches!(s, S::Running | S::Waiting | S::Blocked | S::Review) {
                effects.push(Effect::StopRunner);
            }
            effects.push(Effect::DependencySatisfied);
            next.paused = false;
            next.review.turn = None;
            (
                S::Cancelled,
                Some(StatusReason::Cancelled {
                    text: reason.clone(),
                }),
            )
        }
        (S::Proposed, TicketEvent::Approve { by }) => {
            person_only(*by, "approve")?;
            let (after, before) = match next.wildcard.as_mut() {
                Some(w) => {
                    w.approved = true;
                    (std::mem::take(&mut w.after), std::mem::take(&mut w.before))
                }
                None => (Vec::new(), Vec::new()),
            };
            effects.push(Effect::AddProposedEdges { after, before });
            (S::Backlog, None)
        }
        (S::Proposed, TicketEvent::Reject { reason, by }) => {
            person_only(*by, "reject")?;
            non_empty(reason, "reject")?;
            (
                S::Cancelled,
                Some(StatusReason::Cancelled {
                    text: reason.clone(),
                }),
            )
        }
        _ => return Err(refuse()),
    };

    if status != from {
        next.status_since = now;
    }
    if status != S::Ready {
        next.queued = false;
    }
    next.status = status;
    next.status_reason = reason;
    Ok(Transition {
        ticket: next,
        effects,
    })
}

mod checks;
mod review;
pub use checks::person_only;
use checks::{block, non_empty, run_on_ship};
mod invariants;

pub use invariants::check_invariants;

#[cfg(test)]
mod tests;
