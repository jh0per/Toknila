//! Agent events: what a runner reports about its ticket or milestone, turned into transitions.

use toknila_core::agent::{AgentEvent, ReleaseEvent};
use toknila_core::graph::ChainView;
use toknila_core::hook::{self, Reaction, Sign};
use toknila_core::machine::TicketEvent;
use toknila_core::{
    Actor, BlockedReason, CommentBody, CommentKind, Release, StatusReason, Ticket, TicketId,
    TicketStatus, WaitingReason,
};

use crate::notes::{add_comment, body_from_text, update_subtask};
use crate::{
    AppError, AppResult, UnitOfWork, checks, projects, reviews, threads, wildcards, with_chain,
};

/// An agent holds a grant for one ticket and acts on that ticket only.
pub fn ensure_granted(granted: TicketId, t: &Ticket) -> AppResult<()> {
    if granted == t.id {
        Ok(())
    } else {
        Err(AppError::forbidden(format!(
            "this agent works on another ticket, not {}",
            t.key
        )))
    }
}

/// Applies one event an agent reported about `t`. Returns the ticket the answer is about: `t`,
/// or the new card for `propose`.
pub fn ticket_event<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    t: &Ticket,
    event: AgentEvent,
) -> AppResult<TicketId> {
    ticket_event_at(uow, t, event, None, None)
}

/// [`ticket_event`] for a hand-in whose ship the host looked at: `head` is the commit the ship's
/// branch was at, remembered as the commit handed in; `since` is what changed since the Return
/// this hand-in answers, kept with it.
pub fn ticket_event_at<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    t: &Ticket,
    event: AgentEvent,
    head: Option<String>,
    since: Option<toknila_core::SinceReturn>,
) -> AppResult<TicketId> {
    // A report of work leaves a card that waits on a question or a permission where it is: only
    // the answer closes those. The hand-in goes on, and an open question is overtaken by it.
    match event {
        AgentEvent::Started | AgentEvent::SubtaskDone { .. } | AgentEvent::Note { .. } => {
            react(uow, t, &Sign::of(hook::SIGNAL_MCP))?;
        }
        AgentEvent::Finished { .. } => {
            react(uow, t, &Sign::of(hook::SIGNAL_HAND_IN))?;
        }
        _ => {}
    }
    match event {
        AgentEvent::Propose(card) => {
            let (chain, from) = (t.chain_id.to_string(), t.key.to_string());
            return wildcards::propose(uow, &chain, card, Some(&from));
        }
        AgentEvent::OpenThread { kind, title } => {
            threads::open(uow, t, kind, &title)?;
            Ok(())
        }
        AgentEvent::ThreadMessage { thread, text } => {
            let th = threads::find(uow, t, &thread)?;
            threads::post_message(uow, t, &th, &text).map(|_| ())
        }
        AgentEvent::ThreadVersion {
            thread,
            format,
            body,
        } => {
            let th = threads::find(uow, t, &thread)?;
            let input = threads::NewVersion {
                format,
                body,
                message: None,
            };
            threads::post_version(uow, t, &th, input).map(|_| ())
        }
        AgentEvent::WithdrawThread { thread } => {
            let th = threads::find(uow, t, &thread)?;
            threads::withdraw(uow, t, &th).map(|_| ())
        }
        AgentEvent::Started => with_chain(uow, t.chain_id, |ctx| {
            if t.status == TicketStatus::Ready {
                ctx.ticket_event(t.id, TicketEvent::Start)?;
            } else if !matches!(t.status, TicketStatus::Running | TicketStatus::Waiting) {
                return Err(AppError::conflict(format!(
                    "cannot start a ticket in {:?}",
                    t.status
                )));
            }
            Ok(())
        }),
        AgentEvent::AskHuman {
            text,
            permission,
            detail,
            choices,
        } => {
            if text.trim().is_empty() {
                return Err(AppError::bad_request("ask_human needs a text"));
            }
            let choices = toknila_core::question::check_choices(choices)
                .map_err(|e| AppError::bad_request(e.to_string()))?;
            let reason = AgentEvent::waiting_reason(&text, permission, detail, choices);
            with_chain(uow, t.chain_id, |ctx| {
                Ok::<_, AppError>(ctx.ticket_event(t.id, TicketEvent::AskHuman { reason })?)
            })?;
            // A permission request lives in the journal and the inbox; it is not a comment on
            // the card (a question the agent asks is).
            if permission {
                return Ok(t.id);
            }
            report(uow, t, body_from_text(&text))
        }
        AgentEvent::SubtaskDone { position } => {
            update_subtask(uow, t, position, Some(true), None).map(|_| ())
        }
        AgentEvent::Note { body } => match note_question(uow, t, &body)? {
            Some(text) => ticket_event(
                uow,
                t,
                AgentEvent::AskHuman {
                    text,
                    permission: false,
                    detail: None,
                    choices: Vec::new(),
                },
            )
            .map(|_| ()),
            None => report(uow, t, body),
        },
        AgentEvent::Finished { summary } => {
            toknila_core::handin::check(&summary)
                .map_err(|e| AppError::bad_request(format!("finished refused: {e}")))?;
            let open_threads = threads::open_threads(uow, t)?;
            if open_threads.is_empty() {
                threads::require_design(uow, t)?;
            }
            let mode = reviews::mode_of(uow, t)?;
            let (check, max_rounds) = checks::plan_of(uow, t)?;
            with_chain(uow, t.chain_id, |ctx| {
                let event = TicketEvent::Finished {
                    summary,
                    open_threads,
                    mode,
                    check,
                    max_rounds,
                };
                ctx.ticket_event(t.id, event)?;
                if let Some(mine) = ctx.view.tickets.iter_mut().find(|x| x.id == t.id) {
                    mine.review.handed_in = head.or(mine.review.handed_in.take());
                    // A new hand-in starts the ship's line over: earlier syncs are behind it.
                    mine.review.synced.clear();
                    mine.review.since_return = since;
                }
                Ok(())
            })
        }
        AgentEvent::ReviewVerdict {
            verdict,
            comment,
            items,
        } => {
            let max_rounds = projects::of_ticket(uow, t)?.max_review_rounds;
            with_chain(uow, t.chain_id, |ctx| {
                let event = TicketEvent::ReviewVerdict {
                    verdict,
                    comment,
                    items,
                    max_rounds,
                };
                Ok(ctx.ticket_event(t.id, event)?)
            })
        }
        AgentEvent::Crashed { detail } => with_chain(uow, t.chain_id, |ctx| {
            let reason = BlockedReason::Crashed { detail };
            Ok(ctx.ticket_event(t.id, TicketEvent::Block { reason })?)
        }),
    }
    .map(|()| t.id)
}

/// The question a running card's note really is; `None` for a plain note, or a card that is not
/// Running (it cannot start waiting).
fn note_question<U: UnitOfWork + ?Sized>(
    uow: &U,
    t: &Ticket,
    body: &CommentBody,
) -> AppResult<Option<String>> {
    let running = uow
        .ticket(t.id)?
        .is_some_and(|t| t.status == TicketStatus::Running);
    Ok(running.then(|| AgentEvent::note_question(body)).flatten())
}

/// Whether `event` from `t` is a note that [`ticket_event`] turns into a question, so the caller
/// can tell the agent to use `ask_human`.
pub fn turns_into_question<U: UnitOfWork + ?Sized>(
    uow: &U,
    t: &Ticket,
    event: &AgentEvent,
) -> AppResult<bool> {
    match event {
        AgentEvent::Note { body } => Ok(note_question(uow, t, body)?.is_some()),
        _ => Ok(false),
    }
}

/// Applies one event a release agent reported about `r`.
pub fn release_event<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    r: &Release,
    event: ReleaseEvent,
) -> AppResult<()> {
    match event {
        ReleaseEvent::Finished => {
            with_chain(uow, r.chain_id, |ctx| Ok(ctx.complete_release(r.id, None)?))
        }
    }
}

fn report<U: UnitOfWork + ?Sized>(uow: &mut U, t: &Ticket, body: CommentBody) -> AppResult<()> {
    let view: ChainView = uow
        .chain_view(t.chain_id)?
        .ok_or_else(|| AppError::not_found("chain"))?;
    add_comment(uow, &view, t.id, Actor::Agent, CommentKind::Report, body).map(|_| ())
}

/// A sign of life from the agent (a hook fired). What it does to the card is `hook::react`'s: a
/// question is closed by its answer alone, a permission by its answer or the hook of the prompted
/// call, and only a silent card goes back to Running on any sign of work.
pub fn activity<U: UnitOfWork + ?Sized>(uow: &mut U, t: &Ticket, sign: &Sign<'_>) -> AppResult<()> {
    react(uow, t, sign).map(|_| ())
}

/// Applies what `hook::react` decides for `t` and the sign: the card goes back to Running through
/// the machine, and a question the hand-in overtook is closed with a system comment.
fn react<U: UnitOfWork + ?Sized>(uow: &mut U, t: &Ticket, sign: &Sign<'_>) -> AppResult<Reaction> {
    let Some(now) = uow.ticket(t.id)? else {
        return Ok(Reaction::Stay);
    };
    let waiting = match &now.status_reason {
        Some(StatusReason::Waiting(w)) => Some(w),
        _ => None,
    };
    let reaction = hook::react(now.status, waiting, sign);
    if reaction == Reaction::Stay {
        return Ok(reaction);
    }
    if reaction == Reaction::Overtake
        && let Some(WaitingReason::Question { text, .. }) = waiting
    {
        let gist = "The question was overtaken by the hand-in".to_owned();
        let body = CommentBody {
            gist,
            what: Some(format!(
                "The agent handed in while this question was open: {text}"
            )),
            ..Default::default()
        };
        let view = uow
            .chain_view(t.chain_id)?
            .ok_or_else(|| AppError::not_found("chain"))?;
        let kind = CommentKind::Report;
        add_comment(uow, &view, t.id, Actor::System, kind, body)?;
    }
    with_chain(uow, t.chain_id, |ctx| {
        Ok::<_, AppError>(ctx.ticket_event(t.id, TicketEvent::AgentActive)?)
    })?;
    Ok(reaction)
}
