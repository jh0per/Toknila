//! Runners as use cases: queue the start and stop that core asked for, record what the host
//! did, turn a process exit into a ticket event, and block the tickets whose runner a daemon
//! restart lost. The process itself runs on the host after the command committed
//! ([`crate::external`]); nothing here starts one.

use std::collections::HashSet;

use toknila_core::events::DomainEvent;
use toknila_core::machine::{TicketEvent, person_only};
use toknila_core::{
    Actor, AgentProfile, AgentRole, BlockedReason, Project, Run, Ticket, TicketId, TicketKey,
    TicketStatus,
};

use crate::chain::tell_ticket;
use crate::external::ExternalEffect;
use crate::runner::{RunBrief, RunEnded, Runner, RunnerError};
use crate::{AppError, AppResult, ChainTx, ErrorKind, UnitOfWork, reviews, with_chain};

/// Whether the ticket's agent is a process. The simulated profile starts none.
fn runs_process(project: &Project, ticket: &Ticket) -> bool {
    project.profile_for(ticket) != AgentProfile::SIM
}

/// `EnsureRunner`: queue starting the ticket's agent for after the commit. A simulated ticket
/// has nothing to start.
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
    if runs_process(&project, t) {
        let comments = uow.comments(ticket)?;
        let returned = toknila_core::instructions::Returned::of(&t.review);
        let waiting = toknila_core::instructions::waiting(&comments, returned.as_ref());
        uow.defer(ExternalEffect::StartRunner {
            ticket,
            key: t.key.clone(),
            profile: project.profile_for(t).to_owned(),
            brief: RunBrief {
                title: t.title.clone(),
                prompt: t.prompt.clone(),
                acceptance: t.acceptance.clone(),
                check_command: project.check_command.clone(),
                setup_command: project.setup_command.clone(),
                earlier: toknila_core::instructions::earlier(&comments),
                discussion: crate::discussion::transcript(uow, ticket)?,
                waiting: waiting.texts,
                waiting_ids: waiting.ids,
                returned,
                design_first: t.design_first,
                card_designs: t.designs.clone(),
                design_project: Some(project.id),
                model: t.model.clone().or_else(|| project.worker_model.clone()),
                command_policy: project.command_policy,
                allowed_commands: project.allowed_commands.clone(),
                web_domains: Vec::new(),
                repo_tools: Vec::new(),
                project: None,
                designs: Vec::new(),
                epic: Some((ctx.view.chain.name.clone(), ctx.view.chain.goal.clone())),
                check: None,
            },
        });
    }
    Ok(())
}

/// `StopRunner`: queue stopping the ticket's runner. Stopping a ticket without one is a no-op on
/// the host, so a simulated ticket needs no check here.
pub(crate) fn queue_stop<U: UnitOfWork + ?Sized>(uow: &mut U, ticket: TicketId) {
    uow.defer(ExternalEffect::StopRunner { ticket });
}

/// `StopWorker`: queue stopping the ticket's worker runner; a reviewer keeps running.
pub(crate) fn queue_stop_worker<U: UnitOfWork + ?Sized>(uow: &mut U, ticket: TicketId) {
    uow.defer(ExternalEffect::StopWorker { ticket });
}

/// The host started a runner: journal it. A runner that was kept is not news.
pub(crate) fn started<U: UnitOfWork + ?Sized>(uow: &mut U, run: Run) -> AppResult<()> {
    uow.emit(&DomainEvent::RunnerStarted { run })
}

/// The host could not start the runner: the ticket is blocked with `no_working_dir` when it has no
/// directory, else as crashed, with the reason.
pub(crate) fn start_failed<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: TicketId,
    error: &RunnerError,
) -> AppResult<()> {
    let reason = match error {
        RunnerError::NoWorkingDir => BlockedReason::NoWorkingDir,
        other => BlockedReason::Crashed {
            detail: other.to_string(),
        },
    };
    tell_ticket(uow, ticket, TicketEvent::Block { reason })
}

/// A runner process ended. The journal says so; a ticket whose agent was still working is
/// blocked by how it ended ([`toknila_core::ProcessExit::block_reason`]), unless the host ended
/// it on purpose. A ticket that has moved on (in Review, merged, blocked already) ignores it.
///
/// Returns `true` when it queued a replacement worker run (see [`replace_worker`]).
pub fn ended<U: UnitOfWork + ?Sized>(uow: &mut U, ended: RunEnded) -> AppResult<bool> {
    let RunEnded { run, exit, stopped } = ended;
    let ticket = run.ticket_id;
    let role = run.role;
    if role == AgentRole::Planner {
        // A project's planner is no card's agent: its end moves nothing, and the window learns of
        // it from the terminal stream.
        return Ok(false);
    }
    let reason = exit.block_reason();
    uow.emit(&DomainEvent::RunnerExited {
        run: run.clone(),
        exit: exit.clone(),
    })?;
    if stopped {
        // The host ended it on purpose (a reviewer whose verdict is in, an accepted card): the
        // slot it held is free now that the process is gone, so a waiting card starts.
        let restarted = role == AgentRole::Worker && replace_worker(uow, &run)?;
        free_slot(uow, ticket)?;
        return Ok(restarted);
    }
    if role == AgentRole::Reviewer {
        // A reviewer reports its verdict first and exits after; one that ends while the card
        // still waits for it gave none, and a person decides. Once the card has moved on, the
        // machine refuses this and nothing changes.
        let detail = match reason {
            BlockedReason::Crashed { .. } if exit.code == Some(0) && exit.signal.is_none() => {
                "it exited without a verdict".to_owned()
            }
            BlockedReason::Crashed { detail } => format!("it ended: {detail}"),
            other => other.to_string(),
        };
        reviews::failed(uow, ticket, detail)?;
        return Ok(false);
    }
    tell_ticket(uow, ticket, TicketEvent::Block { reason })?;
    Ok(false)
}

/// A run younger than this that the host stopped gets no second replacement.
const RESTART_GUARD_MS: i64 = 5_000;

/// The safety net for a worker the host stopped: a Running card with no live worker still needs
/// an agent (the replacement the machine asked for may have been lost to a race with this stop),
/// so a run starts again with the kickoff that carries the open Return and the pending
/// instructions. A card in any other state is left alone; a run that ended right after it
/// started blocks the card instead of starting a loop. Returns whether a start was queued.
fn replace_worker<U: UnitOfWork + ?Sized>(uow: &mut U, run: &Run) -> AppResult<bool> {
    let ticket = run.ticket_id;
    let Some(t) = uow.ticket(ticket)? else {
        return Ok(false);
    };
    if t.status != TicketStatus::Running
        || uow.live_runners()?.contains(&(ticket, AgentRole::Worker))
    {
        return Ok(false);
    }
    let Some(view) = uow.chain_view(t.chain_id)? else {
        return Ok(false);
    };
    let Some(project) = uow.project(view.chain.project_id)? else {
        return Ok(false);
    };
    if !runs_process(&project, &t) {
        return Ok(false);
    }
    let started = i64::try_from(run.id.0.timestamp_ms()).unwrap_or(i64::MAX);
    if (0..RESTART_GUARD_MS).contains(&(uow.now().0 - started)) {
        let detail = "the agent ends right after it starts".to_owned();
        let reason = BlockedReason::Crashed { detail };
        tell_ticket(uow, ticket, TicketEvent::Block { reason })?;
        return Ok(false);
    }
    let ctx = ChainTx::new(view, uow.now());
    crate::ships::queue_prepare(uow, &ctx, ticket, Default::default())?;
    queue_start(uow, &ctx, ticket)?;
    Ok(true)
}

/// Gives the slot a runner that has ended held to the cards waiting for one in its project.
fn free_slot<U: UnitOfWork + ?Sized>(uow: &mut U, ticket: TicketId) -> AppResult<()> {
    let Some(chain) = uow
        .ticket(ticket)?
        .and_then(|t| uow.chain(t.chain_id).transpose())
        .transpose()?
    else {
        return Ok(());
    };
    crate::schedule::run(uow, chain.project_id)
}

/// After a daemon start: tickets whose agent is a process, and that were working, lost their
/// runner unless `live` names it. They go to Blocked(`runner_lost`) through the machine; the
/// simulated ones have no process to lose. Returns their keys.
pub fn mark_lost<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    live: &HashSet<TicketId>,
) -> AppResult<Vec<TicketKey>> {
    let mut lost = Vec::new();
    for chain in uow.chains(None)? {
        let Some(project) = uow.project(chain.project_id)? else {
            continue;
        };
        let ids: Vec<(TicketId, TicketKey)> = uow
            .tickets(Some(chain.id))?
            .iter()
            .filter(|t| matches!(t.status, TicketStatus::Running | TicketStatus::Waiting))
            .filter(|t| runs_process(&project, t) && !live.contains(&t.id))
            .map(|t| (t.id, t.key.clone()))
            .collect();
        if ids.is_empty() {
            continue;
        }
        with_chain(uow, chain.id, |tx| {
            for (id, _) in &ids {
                let reason = BlockedReason::RunnerLost;
                tx.ticket_event(*id, TicketEvent::Block { reason })?;
            }
            Ok::<_, AppError>(())
        })?;
        lost.extend(ids.into_iter().map(|(_, key)| key));
    }
    Ok(lost)
}

/// A person types into the terminal of the ticket's runner of `role` (an answer, a nudge, an
/// instruction). Only a person does; the agent and the system have the event contract instead.
pub fn send_input(
    runner: &dyn Runner,
    actor: Actor,
    ticket: &Ticket,
    role: AgentRole,
    bytes: &[u8],
) -> AppResult<()> {
    person_only(actor, "write to the agent's terminal")?;
    if bytes.is_empty() {
        return Err(AppError::bad_request("nothing to write"));
    }
    runner.write(ticket.id, role, bytes).map_err(|e| {
        let kind = match e {
            RunnerError::NotRunning => ErrorKind::Conflict,
            _ => ErrorKind::Internal,
        };
        AppError::new(kind, e.to_string())
    })
}

/// A person's panel changed size: the ticket's terminal follows it, within the bounds core sets.
pub fn resize(
    runner: &dyn Runner,
    actor: Actor,
    ticket: &Ticket,
    cols: u32,
    rows: u32,
) -> AppResult<()> {
    person_only(actor, "resize the agent's terminal")?;
    let size = toknila_core::instructions::terminal::clamp(cols, rows);
    runner.resize(ticket.id, size).map_err(|e| {
        let kind = match e {
            RunnerError::NotRunning => ErrorKind::Conflict,
            _ => ErrorKind::Internal,
        };
        AppError::new(kind, e.to_string())
    })
}
