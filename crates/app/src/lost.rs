//! After a daemon start: the cards whose agent the restart lost.

use std::collections::HashSet;

use toknila_core::machine::TicketEvent;
use toknila_core::{TicketId, TicketKey, TicketStatus};

use crate::runs::runs_process;
use crate::{AppError, AppResult, UnitOfWork, with_chain};

/// After a daemon start: tickets whose agent is a process, and that were working, lost their
/// runner unless `live` names it. A Running card with no run record never got its agent (the
/// daemon restarted before the start landed), so it is lost too, unless its project is simulated.
/// A lost runner is a paused agent, not a blocked card: a Running card wants its agent back and the
/// schedule starts a replacement run when a slot is free; a Waiting card keeps its question and the
/// answer starts the agent. Lost twice within ten minutes the card is Blocked(`runner_lost`)
/// instead (core decides). The simulated ones have no process to lose. Returns their keys.
pub fn mark_lost<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    live: &HashSet<TicketId>,
) -> AppResult<Vec<TicketKey>> {
    let mut lost = Vec::new();
    for chain in uow.chains(None)? {
        let Some(project) = uow.project(chain.project_id)? else {
            continue;
        };
        let mut ids: Vec<(TicketId, TicketKey, Option<String>)> = Vec::new();
        for t in uow.tickets(Some(chain.id))? {
            if live.contains(&t.id)
                || !matches!(t.status, TicketStatus::Running | TicketStatus::Waiting)
            {
                continue;
            }
            // A paused agent has no process on purpose (or was lost already); its card waits.
            if t.agent_paused.is_some() {
                continue;
            }
            let run = uow.last_snapshot_payload("ticket", &t.id.to_string(), "run")?;
            let unrun = t.status == TicketStatus::Running && !project.simulated && run.is_none();
            if runs_process(&project, &t) || unrun {
                // The resume id of the run, when its record says one.
                let session = run
                    .as_ref()
                    .and_then(|r| r.get("session"))
                    .and_then(|s| s.as_str())
                    .map(str::to_owned);
                ids.push((t.id, t.key.clone(), session));
            }
        }
        if ids.is_empty() {
            continue;
        }
        with_chain(uow, chain.id, |tx| {
            for (id, _, session) in &ids {
                let session = session.clone();
                tx.ticket_event(*id, TicketEvent::RunnerLost { session })?;
            }
            Ok::<_, AppError>(())
        })?;
        lost.extend(ids.into_iter().map(|(_, key, _)| key));
    }
    Ok(lost)
}
