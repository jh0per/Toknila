//! The silence loop as use cases: look at a live run and decide ([`check`], over the pure rule in
//! `toknila_core::nudge`), then carry the decision out ([`apply`]). A nudge is an instruction
//! comment from the system, so it travels the same way a person's does (typed into an idle agent
//! or handed to its Stop hook, journaled as `comment.delivered`). Giving up moves the card to
//! Waiting (silent) through the machine.
//!
//! The nudge history is the comments themselves: it survives a daemon restart and the owner sees
//! each nudge on the card.

use toknila_core::machine::TicketEvent;
use toknila_core::nudge::{self, Decision, Signals};
use toknila_core::{
    Actor, CommentBody, CommentKind, Nudge, Ticket, TicketId, TicketStatus, Timestamp,
    WaitingReason,
};

use crate::chain::tell_ticket;
use crate::notes::add_comment;
use crate::runner::RunnerState;
use crate::{AppError, AppResult, Reader, UnitOfWork};

/// What the host knows of a run's signs of life besides its output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reported {
    /// When the run started.
    pub started: Timestamp,
    /// Its last MCP call.
    pub mcp: Option<Timestamp>,
    /// Its last hook.
    pub hook: Option<Timestamp>,
    /// A `screen` question is open for the run (the host sets it; it is not a sign of life).
    pub screen_open: bool,
}

/// Whether a comment is a nudge: an instruction the system wrote.
fn is_nudge(c: &toknila_core::Comment) -> bool {
    c.author == Actor::System && c.kind == CommentKind::Instruction
}

/// Decides what to do about the ticket's live run at `now`; `live` is what the host says of the
/// process (`Runner::state`, asked by the caller so nothing slow runs here). A ticket that is not
/// Running, or has no live process, is left alone.
pub fn check<R: Reader + ?Sized>(
    r: &R,
    live: Option<RunnerState>,
    ticket: TicketId,
    cfg: &Nudge,
    reported: Reported,
    now: Timestamp,
) -> AppResult<Decision> {
    let running = r
        .ticket(ticket)?
        .is_some_and(|t| t.status == TicketStatus::Running);
    let Some(state) = live.filter(|_| running) else {
        return Ok(Decision::Wait);
    };
    let comments = r.comments(ticket)?;
    let nudges: Vec<Timestamp> = comments
        .iter()
        .filter(|c| is_nudge(c))
        .map(|c| c.created_at)
        .collect();
    let delivered = comments
        .iter()
        .filter(|c| c.kind == CommentKind::Instruction)
        .filter_map(|c| c.delivered_at)
        .max();
    let quiet = i64::try_from(state.quiet_ms).unwrap_or(i64::MAX);
    let signals = Signals {
        started: reported.started,
        mcp: reported.mcp,
        hook: reported.hook,
        output: Some(Timestamp(now.0.saturating_sub(quiet))),
        delivered,
        screen_open: reported.screen_open,
    };
    Ok(nudge::decide(cfg, &signals, state.busy, &nudges, now))
}

/// Carries out `decision` for the ticket, if it is still Running.
pub fn apply<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: TicketId,
    cfg: &Nudge,
    decision: Decision,
) -> AppResult<()> {
    let Some(t) = uow
        .ticket(ticket)?
        .filter(|t| t.status == TicketStatus::Running)
    else {
        return Ok(());
    };
    match decision {
        Decision::Wait => Ok(()),
        Decision::Nudge => send(uow, &t, cfg),
        Decision::GiveUp => {
            let reason = WaitingReason::Silent;
            tell_ticket(uow, ticket, TicketEvent::AskHuman { reason })
        }
    }
}

/// The nudge as an instruction comment from the system; delivery is the instructions' business.
fn send<U: UnitOfWork + ?Sized>(uow: &mut U, t: &Ticket, cfg: &Nudge) -> AppResult<()> {
    let view = uow
        .chain_view(t.chain_id)?
        .ok_or_else(|| AppError::not_found("chain"))?;
    let body = CommentBody {
        gist: cfg.message.clone(),
        ..CommentBody::default()
    };
    add_comment(
        uow,
        &view,
        t.id,
        Actor::System,
        CommentKind::Instruction,
        body,
    )
    .map(|_| ())
}
