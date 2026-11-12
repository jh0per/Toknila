//! A card that waits for a person holds no slot: once it has waited for the grace, the daemon
//! records that its agent is paused (the session goes with the card, `toknila_app::paused`) and
//! ends the agent's process, saying so in the run log. The answer starts a new run
//! ([`toknila_app::runs`]); the silence detector's own rules are not touched.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use toknila_app::{AppError, paused};
use toknila_core::{Actor, AgentRole, StatusReason, TicketStatus, Timestamp, WaitingReason};

use super::Runners;
use crate::store::Store;

/// How long a card waits for a person before its agent's process is ended.
pub(super) const GRACE: Duration = Duration::from_secs(60);

impl Runners {
    /// How long a card waits before its agent is paused.
    pub fn pause_grace(&self) -> Duration {
        Duration::from_millis(self.pause_grace_ms.load(Ordering::Relaxed))
    }

    /// Sets how long a card waits before its agent is paused (tests shorten it).
    pub fn set_pause_grace(&self, grace: Duration) {
        let ms = u64::try_from(grace.as_millis()).unwrap_or(u64::MAX);
        self.pause_grace_ms.store(ms, Ordering::Relaxed);
    }

    /// Ends the ticket's worker process on purpose; `note` is the last line of its run log.
    fn pause_worker(&self, ticket: toknila_core::TicketId, note: String) {
        let active = self.active().get(&(ticket, AgentRole::Worker)).cloned();
        if let Some(a) = active {
            a.kill_with_note(note);
        }
    }
}

/// One round at time `at`: the cards that have waited for the grace with their agent alive are
/// paused.
pub async fn tick(store: &Store, runners: &Arc<Runners>, at: Timestamp) {
    let grace = i64::try_from(runners.pause_grace().as_millis()).unwrap_or(i64::MAX);
    for id in runners.live() {
        let Ok(Ok(Some(t))) = store
            .read(move |repo| Ok(toknila_app::Reader::ticket(repo, id)))
            .await
        else {
            continue;
        };
        let waited = at.0.saturating_sub(t.status_since.0);
        if t.status != TicketStatus::Waiting || t.agent_paused.is_some() || waited < grace {
            continue;
        }
        // The screen is the process's own: ending it would lose the question it holds.
        if t.status_reason == Some(StatusReason::Waiting(WaitingReason::Screen)) {
            continue;
        }
        let session = runners.prompts().session(id, AgentRole::Worker);
        let done = store
            .write(Actor::System, at, move |tx| -> Result<bool, AppError> {
                paused::pause(tx, id, session)?;
                // The card may have moved on since it was read: only a paused card loses its agent.
                Ok(toknila_app::Reader::ticket(tx, id)?.is_some_and(|t| t.agent_paused.is_some()))
            })
            .await;
        match done {
            Ok((true, _)) => {
                runners.pause_worker(id, format!("paused: waiting for {}", t.waiting_for()));
            }
            Ok(_) => {}
            Err(error) => tracing::warn!(ticket = %t.key, %error, "cannot pause the agent"),
        }
    }
}
