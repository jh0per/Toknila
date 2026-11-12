//! The startup watch: a run that shows no sign of life for its profile's `startup_secs` after it
//! was spawned waits on its screen (core's rule, `toknila_core::question`). The inbox computes the
//! item from the same rule at every request; this notices the moment it starts and ends, so the
//! journal says so in words (`question_raised`, `question_cleared`), a worker's card moves to
//! Waiting (reason: its screen) and back, and open windows refresh.

use std::collections::HashMap;
use std::sync::Arc;

use toknila_app::{AppError, Reader, questions};
use toknila_core::question::{self, SCREEN_LINES};
use toknila_core::{Actor, AgentRole, TicketId, Timestamp};

use super::Runners;
use crate::store::Store;

/// The keys that choose Yes or No on the trust dialog the run shows now, or `None` when its screen
/// is another one (a person answers that in the card's Terminal).
pub fn trust_keys(
    runners: &Runners,
    ticket: TicketId,
    role: AgentRole,
    yes: bool,
    at: Timestamp,
) -> Option<Vec<u8>> {
    let live = runners.facts(at);
    let run = live
        .runs
        .iter()
        .find(|r| r.ticket == ticket && r.role == role)?;
    question::trust_keys(&question::last_lines(&run.tail, SCREEN_LINES), yes)
}

/// One round at time `at`: journals the runs that began or stopped waiting on their screen.
pub async fn tick(store: &Store, runners: &Arc<Runners>, at: Timestamp) {
    let live = runners.facts(at);
    let waiting: HashMap<(TicketId, AgentRole), Vec<String>> = live
        .runs
        .iter()
        .filter(|r| r.waits_on_screen(at))
        .map(|r| {
            let lines = question::last_lines(&r.tail, SCREEN_LINES);
            ((r.ticket, r.role), lines)
        })
        .collect();
    let prompts = runners.prompts();
    for (&(ticket, role), lines) in &waiting {
        if prompts.flag_screen(ticket, role) {
            journal(store, ticket, role, at, Some(lines.clone())).await;
        }
    }
    for (ticket, role) in prompts.flagged() {
        if !waiting.contains_key(&(ticket, role)) && prompts.unflag_screen(ticket, role) {
            journal(store, ticket, role, at, None).await;
        }
    }
}

/// `lines` is the screen when the question is raised, `None` when it goes away.
async fn journal(
    store: &Store,
    ticket: TicketId,
    role: AgentRole,
    at: Timestamp,
    lines: Option<Vec<String>>,
) {
    let lines_for_log = lines.clone();
    let done = store
        .write(Actor::System, at, move |tx| -> Result<(), AppError> {
            let Some(t) = tx.ticket(ticket)? else {
                return Ok(());
            };
            match &lines {
                Some(lines) => questions::screen_raised(tx, &t, role, lines),
                None => questions::screen_cleared(tx, &t, role),
            }
        })
        .await;
    match (done, &lines_for_log) {
        (Err(error), _) => tracing::warn!(%ticket, %error, "cannot journal the startup wait"),
        (Ok(_), Some(lines)) => tracing::info!(%ticket, "{}", question::screen_words(lines)),
        (Ok(_), None) => tracing::info!(%ticket, "the agent's screen question is over"),
    }
}
