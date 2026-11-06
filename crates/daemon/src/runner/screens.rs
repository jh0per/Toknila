//! The startup watch: a run that shows no sign of life for its profile's `startup_secs` after it
//! was spawned waits on its screen (core's rule, `toknila_core::question`). The inbox computes the
//! item from the same rule at every request; this only notices the moment it starts and ends, so
//! the journal says so (`question_raised`, `question_cleared`) and open windows refresh.

use std::collections::HashSet;
use std::sync::Arc;

use toknila_app::{AppError, Reader, questions};
use toknila_core::question::QuestionKind;
use toknila_core::{Actor, AgentRole, TicketId, Timestamp};

use super::Runners;
use crate::store::Store;

/// One round at time `at`: journals the runs that began or stopped waiting on their screen.
pub async fn tick(store: &Store, runners: &Arc<Runners>, at: Timestamp) {
    let live = runners.facts(at);
    let waiting: HashSet<(TicketId, AgentRole)> = live
        .runs
        .iter()
        .filter(|r| r.waits_on_screen(at))
        .map(|r| (r.ticket, r.role))
        .collect();
    let prompts = runners.prompts();
    for &(ticket, role) in &waiting {
        if prompts.flag_screen(ticket, role) {
            journal(store, ticket, role, at, true).await;
        }
    }
    for (ticket, role) in prompts.flagged() {
        if !waiting.contains(&(ticket, role)) && prompts.unflag_screen(ticket, role) {
            journal(store, ticket, role, at, false).await;
        }
    }
}

async fn journal(store: &Store, ticket: TicketId, role: AgentRole, at: Timestamp, raised: bool) {
    let done = store
        .write(Actor::System, at, move |tx| -> Result<(), AppError> {
            let Some(t) = tx.ticket(ticket)? else {
                return Ok(());
            };
            if raised {
                questions::raised(tx, &t, role, QuestionKind::Screen)
            } else {
                questions::cleared(tx, &t, role)
            }
        })
        .await;
    if let Err(error) = done {
        tracing::warn!(%ticket, %error, "cannot journal the startup wait");
    }
}
