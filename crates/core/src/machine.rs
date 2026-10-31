//! The ticket state machine. Every status change goes through [`apply`]; an event that the
//! transition table does not allow from the current status is a typed [`TransitionError`].
//!
//! | From | To | Event |
//! |---|---|---|
//! | Backlog | Ready | `DepsSatisfied` (not paused) |
//! | Ready | Backlog | `DepsUnsatisfied` |
//! | Ready | Running | `Start` |
//! | Running | Waiting | `AskHuman` |
//! | Waiting | Running | `InstructionDelivered` |
//! | Running | Review | `Finished` |
//! | Running, Waiting | Blocked | `Block` (Review too, for a merge conflict) |
//! | Blocked | previous status | `Unblock` |
//! | Review | Running | `Return` |
//! | Review | Merged | `Accept` |
//! | Merged | Released | `Released` |
//! | Merged | Review | `Reopen` |
//! | Backlog, Ready, Running, Waiting, Blocked | Backlog (paused) | `Pause` |
//! | Backlog (paused) | Backlog | `Resume` |
//! | any but Merged, Released, Cancelled | Cancelled | `Cancel` |
//!
//! Pause is a flag, not a status: a paused ticket sits in Backlog. Graph-wide consequences
//! (dependents becoming Ready, Blocked or Backlog) come back as [`Effect`]s for the chain graph.

use serde::{Deserialize, Serialize};

use crate::{BlockedReason, StatusReason, Ticket, TicketStatus, Timestamp, WaitingReason};

/// Something that happened to a ticket.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum TicketEvent {
    /// Graph: every dependency is satisfied.
    DepsSatisfied,
    /// Graph: a dependency is no longer satisfied (it was reopened).
    DepsUnsatisfied,
    /// Autostart or `tok start`; the runner confirmed it started.
    Start,
    /// Agent: asked a question, wants permission, or went silent.
    AskHuman { reason: WaitingReason },
    /// A person's new comment reached the agent.
    InstructionDelivered,
    /// Agent: done, a diff is ready. The summary becomes a report comment.
    Finished { summary: String },
    /// Agent crash, runner loss, dependency reopened, merge conflict, or `tok block`.
    Block { reason: BlockedReason },
    /// `tok unblock`.
    Unblock,
    /// `tok return` with a comment for the agent.
    Return { comment: String },
    /// `tok accept`.
    Accept,
    /// Graph: the milestone this ticket belongs to was released.
    Released,
    /// `tok reopen`.
    Reopen,
    /// `tok pause`.
    Pause,
    /// `tok resume`.
    Resume,
    /// `tok cancel <reason>`.
    Cancel { reason: String },
}

impl TicketEvent {
    /// Short event name for errors and logs.
    pub fn name(&self) -> &'static str {
        match self {
            Self::DepsSatisfied => "deps_satisfied",
            Self::DepsUnsatisfied => "deps_unsatisfied",
            Self::Start => "start",
            Self::AskHuman { .. } => "ask_human",
            Self::InstructionDelivered => "instruction_delivered",
            Self::Finished { .. } => "finished",
            Self::Block { .. } => "block",
            Self::Unblock => "unblock",
            Self::Return { .. } => "return",
            Self::Accept => "accept",
            Self::Released => "released",
            Self::Reopen => "reopen",
            Self::Pause => "pause",
            Self::Resume => "resume",
            Self::Cancel { .. } => "cancel",
        }
    }
}

/// Work the caller must do after a transition. Core decides; the daemon and graph execute.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "effect", rename_all = "snake_case")]
pub enum Effect {
    /// Make sure a runner is working on the ticket (start or restart it).
    EnsureRunner,
    /// Stop the ticket's runner, if any.
    StopRunner,
    /// Store the agent's summary as a report comment.
    RecordReport { summary: String },
    /// Store a person's text as an instruction comment for the agent.
    RecordInstruction { text: String },
    /// The ticket now counts as done for its dependents (Merged or Cancelled): recompute Ready.
    DependencySatisfied,
    /// The ticket left Merged: dependents in Running go Blocked, in Ready go back to Backlog.
    DependencyReopened,
    /// The ticket may have become startable (resumed): recompute its readiness.
    RecheckReadiness,
}

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
    #[error("ticket is not paused")]
    NotPaused,
    #[error("{0} needs a non-empty text")]
    EmptyText(&'static str),
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
        (S::Ready, TicketEvent::Start) => {
            effects.push(Effect::EnsureRunner);
            (S::Running, None)
        }
        (S::Running, TicketEvent::AskHuman { reason }) => {
            (S::Waiting, Some(StatusReason::Waiting(reason.clone())))
        }
        (S::Waiting, TicketEvent::InstructionDelivered) => (S::Running, None),
        (S::Running, TicketEvent::Finished { summary }) => {
            effects.push(Effect::RecordReport {
                summary: summary.clone(),
            });
            (S::Review, None)
        }
        (S::Running | S::Waiting, TicketEvent::Block { reason }) => {
            block(reason, ticket, &mut effects)?
        }
        (
            S::Review,
            TicketEvent::Block {
                reason: reason @ BlockedReason::MergeConflict { .. },
            },
        ) => block(reason, ticket, &mut effects)?,
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
                    effects.push(Effect::EnsureRunner);
                    (S::Running, None)
                }
                (other, _) => (other, None),
            }
        }
        (S::Review, TicketEvent::Return { comment }) => {
            non_empty(comment, "return")?;
            effects.push(Effect::RecordInstruction {
                text: comment.clone(),
            });
            effects.push(Effect::EnsureRunner);
            (S::Running, None)
        }
        (S::Review, TicketEvent::Accept) => {
            effects.push(Effect::StopRunner);
            effects.push(Effect::DependencySatisfied);
            (S::Merged, None)
        }
        (S::Merged, TicketEvent::Released) => (S::Released, None),
        (S::Merged, TicketEvent::Reopen) => {
            effects.push(Effect::DependencyReopened);
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
        (_, TicketEvent::Resume) => {
            if !ticket.paused {
                return Err(TransitionError::NotPaused);
            }
            next.paused = false;
            effects.push(Effect::RecheckReadiness);
            (from, ticket.status_reason.clone())
        }
        (s, TicketEvent::Cancel { reason })
            if !matches!(s, S::Merged | S::Released | S::Cancelled) =>
        {
            non_empty(reason, "cancel")?;
            if matches!(s, S::Running | S::Waiting | S::Blocked | S::Review) {
                effects.push(Effect::StopRunner);
            }
            effects.push(Effect::DependencySatisfied);
            next.paused = false;
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
    next.status = status;
    next.status_reason = reason;
    Ok(Transition {
        ticket: next,
        effects,
    })
}

fn block(
    reason: &BlockedReason,
    ticket: &Ticket,
    effects: &mut Vec<Effect>,
) -> Result<(TicketStatus, Option<StatusReason>), TransitionError> {
    if let BlockedReason::Manual { text } = reason {
        non_empty(text, "block")?;
    }
    if matches!(
        reason,
        BlockedReason::Crashed { .. } | BlockedReason::RunnerLost
    ) {
        effects.push(Effect::StopRunner);
    }
    Ok((
        TicketStatus::Blocked,
        Some(StatusReason::Blocked {
            reason: reason.clone(),
            resume_to: ticket.status,
            waiting: match &ticket.status_reason {
                Some(StatusReason::Waiting(w)) => Some(w.clone()),
                _ => None,
            },
        }),
    ))
}

fn non_empty(text: &str, what: &'static str) -> Result<(), TransitionError> {
    if text.trim().is_empty() {
        Err(TransitionError::EmptyText(what))
    } else {
        Ok(())
    }
}

/// Invariants every ticket must satisfy. Returns the first violation.
pub fn check_invariants(ticket: &Ticket) -> Result<(), String> {
    use TicketStatus as S;
    match (ticket.status, &ticket.status_reason) {
        (S::Waiting, Some(StatusReason::Waiting(_))) => {}
        (
            S::Blocked,
            Some(StatusReason::Blocked {
                resume_to, waiting, ..
            }),
        ) => {
            if !matches!(resume_to, S::Running | S::Waiting | S::Review) {
                return Err(format!("blocked ticket resumes to {resume_to:?}"));
            }
            if waiting.is_some() != (*resume_to == S::Waiting) {
                return Err(format!(
                    "blocked ticket resumes to {resume_to:?} with {waiting:?}"
                ));
            }
        }
        (S::Cancelled, Some(StatusReason::Cancelled { .. })) => {}
        (S::Waiting | S::Blocked | S::Cancelled, r) => {
            return Err(format!("{:?} with reason {r:?}", ticket.status));
        }
        (_, None) => {}
        (s, Some(r)) => return Err(format!("{s:?} must not carry a reason, has {r:?}")),
    }
    if ticket.paused && ticket.status != S::Backlog {
        return Err(format!("paused ticket in {:?}", ticket.status));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
