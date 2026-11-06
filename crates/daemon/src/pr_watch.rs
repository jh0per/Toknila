//! The pull-request ticker: every few seconds each card whose merge goes through a pull request
//! gets that pull request looked at (`toknila_app::merges::poll`). The look runs after the
//! commit like any host work, and what `gh` found returns as an event through the machine: the
//! checks refreshed, the card merged, or blocked with the failing checks and the link.

use std::time::Duration;

use tokio::task::JoinHandle;
use toknila_app::{AppError, merges};
use toknila_core::Actor;

use crate::server::now;
use crate::store::Store;

/// How often open pull requests are looked at.
const EVERY: Duration = Duration::from_secs(20);

/// Looks at the open pull requests until the daemon ends.
pub fn watch(store: Store) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut timer = tokio::time::interval(EVERY);
        loop {
            timer.tick().await;
            tick(&store).await;
        }
    })
}

/// One round: every watched pull request is looked at once. Returns how many were.
pub async fn tick(store: &Store) -> usize {
    match store
        .write(Actor::System, now(), |tx| -> Result<_, AppError> {
            merges::poll(tx)
        })
        .await
    {
        Ok((asked, _)) => asked.len(),
        Err(error) => {
            tracing::warn!(%error, "looking at the pull requests failed");
            0
        }
    }
}
