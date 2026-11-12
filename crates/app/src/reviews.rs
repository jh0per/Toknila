//! Review as use cases: who reviews a ticket and with which agent, the reviewer's runner, the
//! reviewer's verdict, and the settings that choose them (ticket, chain, project).
//!
//! The reviewer runs on its own runner next to the worker's, on the same ship. The worker's
//! runner is never replaced, so a return after review goes to a session that still has its
//! context. A reviewer is one-shot: it reports its verdict through the `review_verdict` event
//! and exits. One that ends without a verdict, or never starts, hands the card to a person.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use toknila_core::instructions::BriefThread;
use toknila_core::machine::TicketEvent;
use toknila_core::{
    Actor, AgentProfile, Chain, CheckPhase, Comment, CommentKind, Project, ReviewMode, ReviewRound,
    ReviewTurn, Ticket, TicketId, TicketKey, TicketStatus, resolve_review_mode, resolve_reviewer,
};

use crate::chain::tell_ticket;
use crate::external::ExternalEffect;
use crate::runner::{RunBrief, RunnerError};
use crate::{AppError, AppResult, ChainTx, Reader, UnitOfWork};

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
    /// The project's round limit of the loop, and how many returns the loop has used since a
    /// person last decided.
    pub max_rounds: u32,
    pub used_rounds: u32,
    /// Every verdict so far, oldest first.
    pub rounds: Vec<ReviewRound>,
    /// Where the card's check stands in the machine's line of checks: 0 it runs now, n it is
    /// the n-th that waits for another check to end (1 is next). Absent when no check is in the
    /// line; only the daemon knows the line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_queue: Option<u32>,
    /// While the check runs: `queued` with its place in the line, `syncing` with main, or
    /// `running` the check command with its start time. Absent before the checker reports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<CheckPhase>,
}

fn context<R: Reader + ?Sized>(r: &R, t: &Ticket) -> AppResult<(Chain, Project)> {
    let chain = r
        .chain(t.chain_id)?
        .ok_or_else(|| AppError::not_found("epic of the ticket"))?;
    let project = r
        .project(chain.project_id)?
        .ok_or_else(|| AppError::not_found("project of the epic"))?;
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
        max_rounds: project.max_review_rounds,
        used_rounds: t.review.auto_returns(),
        rounds: t.review.rounds.clone(),
        check_queue: None,
        check: t.review.check.clone(),
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
        .ok_or_else(|| AppError::not_found("project of the epic"))?;
    let profile = resolve_reviewer(t, &ctx.view.chain, &project);
    if profile == AgentProfile::SIM_REVIEWER {
        return Ok(());
    }
    let dir = uow.ship(ticket)?.map(|s| s.path);
    let threads = BriefThread::of(&uow.threads(ticket)?);
    let hand_in = latest_hand_in(&uow.comments(ticket)?);
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
            earlier: Vec::new(),
            // A hand-in that answers a Return names what changed since it, to read first.
            returned: toknila_core::instructions::Returned::of(&t.review),
            model: project.reviewer_model.clone(),
            web_domains: project.reviewer_web_domains.clone(),
            project: None,
            // The daemon's own run of the gate on this hand-in, so the reviewer does not rerun it.
            check: t.review.last_check.clone(),
            // What the reviewer cannot open: the card's threads and the whole hand-in.
            threads,
            hand_in,
            ..RunBrief::default()
        },
    });
    Ok(())
}

/// The worker's latest hand-in on the card, whole; `None` when it gave none.
fn latest_hand_in(comments: &[Comment]) -> Option<String> {
    comments
        .iter()
        .filter(|c| c.kind == CommentKind::Report && c.author == Actor::Agent)
        .filter(|c| toknila_core::handin::from_body(&c.body).is_some())
        .max_by_key(|c| c.created_at)
        .and_then(|c| toknila_core::handin::from_body(&c.body))
}

/// `StopReviewer`: queue stopping the reviewer's runner; the worker's keeps running.
pub(crate) fn queue_stop<U: UnitOfWork + ?Sized>(uow: &mut U, ticket: TicketId) {
    uow.defer(ExternalEffect::StopReviewer { ticket });
}

/// `NudgeReviewer`: queue telling the live reviewer to record its verdict.
pub(crate) fn queue_nudge<U: UnitOfWork + ?Sized>(uow: &mut U, ticket: TicketId, message: String) {
    uow.defer(ExternalEffect::NudgeReviewer { ticket, message });
}

/// The reviewer ended a turn without recording a verdict (`screen` is the end of its screen,
/// ANSI removed). Core decides: the first time it is nudged, the second time the card goes to a
/// person with a note. A card that has moved on ignores it.
pub fn stopped<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: TicketId,
    screen: Vec<String>,
) -> AppResult<()> {
    tell_ticket(uow, ticket, TicketEvent::ReviewerStopped { screen })
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
