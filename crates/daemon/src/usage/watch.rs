//! The usage ticker: keeps the store close to the transcripts, so the cards' figures follow the
//! agents without a page asking.

use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinHandle;

use super::{Scope, Usage};
use crate::store::Store;

/// How often transcripts are read again.
const EVERY: Duration = Duration::from_secs(30);

/// Reads new transcript lines until the daemon ends; the first pass runs at once.
pub fn watch(store: Store, usage: Arc<Usage>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut timer = tokio::time::interval(EVERY);
        loop {
            timer.tick().await;
            if let Err(error) = usage.refresh(&store, Scope::All).await {
                tracing::warn!(%error, "reading transcripts failed");
            }
        }
    })
}
