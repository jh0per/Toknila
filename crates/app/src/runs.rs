//! Runners as use cases: queue the start and stop that core asked for, record what the host
//! did, turn a process exit into a ticket event, and block the tickets whose runner a daemon
//! restart lost. The process itself runs on the host after the command committed
//! ([`crate::external`]); nothing here starts one.

use std::collections::HashSet;

use toknila_core::events::DomainEvent;
use toknila_core::machine::{TicketEvent, person_only};
use toknila_core::{
    Actor, AgentProfile, BlockedReason, Project, Run, Ticket, TicketId, TicketKey, TicketStatus,
};

use crate::chain::tell_ticket;
use crate::external::ExternalEffect;
use crate::runner::{RunEnded, Runner, RunnerError};
use crate::{AppError, AppResult, ChainTx, ErrorKind, UnitOfWork, with_chain};

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
        uow.defer(ExternalEffect::StartRunner {
            ticket,
            key: t.key.clone(),
            profile: project.profile_for(t).to_owned(),
        });
    }
    Ok(())
}

/// `StopRunner`: queue stopping the ticket's runner. Stopping a ticket without one is a no-op on
/// the host, so a simulated ticket needs no check here.
pub(crate) fn queue_stop<U: UnitOfWork + ?Sized>(uow: &mut U, ticket: TicketId) {
    uow.defer(ExternalEffect::StopRunner { ticket });
}

/// The host started a runner: journal it. A runner that was kept is not news.
pub(crate) fn started<U: UnitOfWork + ?Sized>(uow: &mut U, run: Run) -> AppResult<()> {
    uow.emit(&DomainEvent::RunnerStarted { run })
}

/// The host could not start the runner: the ticket is blocked as crashed, with the reason.
pub(crate) fn start_failed<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: TicketId,
    error: &RunnerError,
) -> AppResult<()> {
    let reason = BlockedReason::Crashed {
        detail: error.to_string(),
    };
    tell_ticket(uow, ticket, TicketEvent::Block { reason })
}

/// A runner process ended. The journal says so; a ticket whose agent was still working is
/// blocked by how it ended ([`toknila_core::ProcessExit::block_reason`]), unless the host ended
/// it on purpose. A ticket that has moved on (in Review, merged, blocked already) ignores it.
pub fn ended<U: UnitOfWork + ?Sized>(uow: &mut U, ended: RunEnded) -> AppResult<()> {
    let RunEnded { run, exit, stopped } = ended;
    let ticket = run.ticket_id;
    let reason = exit.block_reason();
    uow.emit(&DomainEvent::RunnerExited { run, exit })?;
    if stopped {
        return Ok(());
    }
    tell_ticket(uow, ticket, TicketEvent::Block { reason })
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

/// A person types into the ticket's terminal (an answer, a nudge, an instruction). Only a person
/// does; the agent and the system have the event contract instead.
pub fn send_input(
    runner: &dyn Runner,
    actor: Actor,
    ticket: &Ticket,
    bytes: &[u8],
) -> AppResult<()> {
    person_only(actor, "write to the agent's terminal")?;
    if bytes.is_empty() {
        return Err(AppError::bad_request("nothing to write"));
    }
    runner.write(ticket.id, bytes).map_err(|e| {
        let kind = match e {
            RunnerError::NotRunning => ErrorKind::Conflict,
            _ => ErrorKind::Internal,
        };
        AppError::new(kind, e.to_string())
    })
}
