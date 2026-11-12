//! After a daemon start: the cards whose agent the restart lost.

use std::collections::HashSet;

use toknila_core::machine::TicketEvent;
use toknila_core::{BlockedReason, TicketId, TicketKey, TicketStatus};

use crate::runs::runs_process;
use crate::{AppError, AppResult, UnitOfWork, with_chain};

/// After a daemon start: tickets whose agent is a process, and that were working, lost their
/// runner unless `live` names it. A Running card with no run record never got its agent (the
/// daemon restarted before the start landed), so it is lost too, unless its project is simulated.
/// They go to Blocked(`runner_lost`) through the machine; the simulated ones have no process to
/// lose. Returns their keys.
pub fn mark_lost<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    live: &HashSet<TicketId>,
) -> AppResult<Vec<TicketKey>> {
    let mut lost = Vec::new();
    for chain in uow.chains(None)? {
        let Some(project) = uow.project(chain.project_id)? else {
            continue;
        };
        let mut ids: Vec<(TicketId, TicketKey)> = Vec::new();
        for t in uow.tickets(Some(chain.id))? {
            if live.contains(&t.id)
                || !matches!(t.status, TicketStatus::Running | TicketStatus::Waiting)
            {
                continue;
            }
            // A paused agent has no process on purpose; its card waits for an answer.
            if t.agent_paused.is_some() {
                continue;
            }
            let unrun = t.status == TicketStatus::Running
                && !project.simulated
                && uow
                    .last_snapshot_payload("ticket", &t.id.to_string(), "run")?
                    .is_none();
            if runs_process(&project, &t) || unrun {
                ids.push((t.id, t.key.clone()));
            }
        }
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
