//! Review as use cases: who reviews a ticket and with which agent, the reviewer's runner, the
//! reviewer's verdict, and the settings that choose them (ticket, chain, project).
//!
//! The reviewer runs on its own runner next to the worker's, on the same ship. The worker's
//! runner is never replaced, so a return after review goes to a session that still has its
//! context. A reviewer is one-shot: it reports its verdict through the `review_verdict` event
//! and exits. One that ends without a verdict, or never starts, hands the card to a person.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use toknila_core::machine::TicketEvent;
use toknila_core::{
    AgentProfile, Chain, Project, ReviewMode, ReviewRound, ReviewTurn, Ticket, TicketId, TicketKey,
    TicketStatus, resolve_review_mode, resolve_reviewer,
};

use crate::chain::tell_ticket;
use crate::external::ExternalEffect;
use crate::runner::{RunBrief, RunnerError};
use crate::{AppError, AppResult, ChainTx, Reader, UnitOfWork, resolve, with_chain};

/// How a ticket is reviewed now, and what has been decided so far.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewView {
    /// The review mode that applies: the ticket's, the chain's or the project's.
    pub mode: ReviewMode,
    /// The reviewer agent profile that applies.
    pub reviewer: String,
    /// Whose verdict the card waits for; `None` outside Review.
    pub awaiting: Option<ReviewTurn>,
    /// Every verdict so far, oldest first.
    pub rounds: Vec<ReviewRound>,
}

fn context<R: Reader + ?Sized>(r: &R, t: &Ticket) -> AppResult<(Chain, Project)> {
    let chain = r
        .chain(t.chain_id)?
        .ok_or_else(|| AppError::not_found("chain of the ticket"))?;
    let project = r
        .project(chain.project_id)?
        .ok_or_else(|| AppError::not_found("project of the chain"))?;
    Ok((chain, project))
}

/// The review mode that applies to `t`: its own, else its chain's, else its project's.
pub fn mode_of<R: Reader + ?Sized>(r: &R, t: &Ticket) -> AppResult<ReviewMode> {
    let (chain, project) = context(r, t)?;
    Ok(resolve_review_mode(t, &chain, &project))
}

/// The review of `t` as a client shows it.
pub fn view<R: Reader + ?Sized>(r: &R, t: &Ticket) -> AppResult<ReviewView> {
    let (chain, project) = context(r, t)?;
    Ok(ReviewView {
        mode: resolve_review_mode(t, &chain, &project),
        reviewer: resolve_reviewer(t, &chain, &project),
        awaiting: t.review.turn,
        rounds: t.review.rounds.clone(),
    })
}

/// `StartReviewer`: queue the reviewer's runner for after the commit. The simulated reviewer
/// has no process: a person sends its verdict (`tok sim review`).
pub(crate) fn queue_start<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ctx: &ChainTx,
    ticket: TicketId,
) -> AppResult<()> {
    let Some(t) = ctx.view.tickets.iter().find(|t| t.id == ticket) else {
        return Ok(());
    };
    let project = uow
        .project(ctx.view.chain.project_id)?
        .ok_or_else(|| AppError::not_found("project of the chain"))?;
    let profile = resolve_reviewer(t, &ctx.view.chain, &project);
    if profile == AgentProfile::SIM_REVIEWER {
        return Ok(());
    }
    let dir = uow.ship(ticket)?.map(|s| s.path);
    uow.defer(ExternalEffect::StartReviewer {
        ticket,
        key: t.key.clone(),
        profile,
        dir,
        brief: RunBrief {
            title: t.title.clone(),
            prompt: t.prompt.clone(),
            acceptance: t.acceptance.clone(),
            check_command: project.check_command.clone(),
        },
    });
    Ok(())
}

/// `StopReviewer`: queue stopping the reviewer's runner; the worker's keeps running.
pub(crate) fn queue_stop<U: UnitOfWork + ?Sized>(uow: &mut U, ticket: TicketId) {
    uow.defer(ExternalEffect::StopReviewer { ticket });
}

/// The reviewer could not start, or ended without a verdict: the card is for a person.
pub(crate) fn failed<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: TicketId,
    detail: String,
) -> AppResult<()> {
    tell_ticket(uow, ticket, TicketEvent::ReviewerFailed { detail })
}

/// The host could not start the reviewer.
pub(crate) fn start_failed<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: TicketId,
    error: &RunnerError,
) -> AppResult<()> {
    failed(uow, ticket, error.to_string())
}

/// After a daemon start: a card still waiting for a reviewer whose process is gone (`live`
/// names the tickets whose reviewer runs) goes to a person. A simulated reviewer has no
/// process and keeps waiting for its verdict. Returns the keys of the cards that moved.
pub fn mark_lost<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    live: &HashSet<TicketId>,
) -> AppResult<Vec<TicketKey>> {
    let mut moved = Vec::new();
    for chain in uow.chains(None)? {
        let Some(project) = uow.project(chain.project_id)? else {
            continue;
        };
        let lost: Vec<(TicketId, TicketKey)> = uow
            .tickets(Some(chain.id))?
            .iter()
            .filter(|t| t.status == TicketStatus::Review)
            .filter(|t| matches!(t.review.turn, Some(ReviewTurn::Agent { .. })))
            .filter(|t| resolve_reviewer(t, &chain, &project) != AgentProfile::SIM_REVIEWER)
            .filter(|t| !live.contains(&t.id))
            .map(|t| (t.id, t.key.clone()))
            .collect();
        for (id, key) in lost {
            tell_ticket(
                uow,
                id,
                TicketEvent::ReviewerFailed {
                    detail: "the daemon restarted while it was reviewing".to_owned(),
                },
            )?;
            moved.push(key);
        }
    }
    Ok(moved)
}

/// What a person may set about review on a chain (a project takes the same two fields in
/// [`crate::projects::PatchProject`]). A field left out is kept.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct ReviewSettings {
    pub review_mode: Option<ReviewMode>,
    /// Reviewer agent profile; an empty text goes back to `<worker profile>-reviewer`.
    pub reviewer: Option<String>,
}

fn clean(reviewer: String) -> Option<String> {
    Some(reviewer.trim().to_owned()).filter(|r| !r.is_empty())
}

/// Sets how a chain's tickets are reviewed. Returns the chain's id.
pub fn set_chain<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    chain: &str,
    settings: ReviewSettings,
) -> AppResult<toknila_core::ChainId> {
    let c = resolve::chain(uow, chain)?;
    with_chain(uow, c.id, |ctx| -> AppResult<()> {
        ctx.edit_chain(|c| {
            if let Some(m) = settings.review_mode {
                c.review_mode = Some(m);
            }
            if let Some(r) = settings.reviewer {
                c.reviewer = clean(r);
            }
        });
        Ok(())
    })?;
    Ok(c.id)
}
