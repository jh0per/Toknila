//! The silence ticker: every few seconds it looks at each live run (`toknila_app::nudge`), and
//! carries out what the rule decided in one command, so a nudge is delivered after the commit like
//! any instruction and giving up moves the card through the machine.

use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinHandle;
use toknila_app::{AppError, nudge};
use toknila_core::nudge::Decision;
use toknila_core::{Actor, Timestamp};

use super::Runners;
use crate::server::now;
use crate::store::Store;

/// How often live runs are looked at.
const EVERY: Duration = Duration::from_secs(5);

/// Evaluates every live run until the daemon ends.
pub fn watch(store: Store, runners: Arc<Runners>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut timer = tokio::time::interval(EVERY);
        loop {
            timer.tick().await;
            tick(&store, &runners, now()).await;
        }
    })
}

/// One round at time `at`: the runs that are quiet are nudged or given up on.
pub async fn tick(store: &Store, runners: &Arc<Runners>, at: Timestamp) {
    super::screens::tick(store, runners, at).await;
    for ticket in runners.live() {
        if let Err(error) = look(store, runners, ticket, at).await {
            tracing::warn!(%ticket, %error, "the silence check failed");
        }
    }
}

async fn look(
    store: &Store,
    runners: &Arc<Runners>,
    ticket: toknila_core::TicketId,
    at: Timestamp,
) -> Result<(), String> {
    let (Some(cfg), Some(reported)) = (runners.nudge_of(ticket), runners.tokens().reported(ticket))
    else {
        return Ok(());
    };
    // The process list is read here, outside the store, so the writer is never held up by it.
    let live = toknila_app::Runner::state(runners.as_ref(), ticket);
    let cfg_read = cfg.clone();
    let decision = store
        .read(move |repo| Ok(nudge::check(repo, live, ticket, &cfg_read, reported, at)))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    if decision == Decision::Wait {
        return Ok(());
    }
    store
        .write(Actor::System, at, move |tx| -> Result<(), AppError> {
            nudge::apply(tx, ticket, &cfg, decision)
        })
        .await
        .map(|_| ())
        .map_err(|e: AppError| e.to_string())
}
