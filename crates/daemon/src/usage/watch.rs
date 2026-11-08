//! The usage ticker: keeps the store close to the transcripts, so the cards' figures follow the
//! agents without a page asking. Each pass has a small budget; while a backlog is left (the first
//! start on a machine with years of transcripts) the next pass follows soon, otherwise after a rest.

use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinHandle;

use super::{Scope, Usage};
use crate::store::Store;

/// How long to rest when everything is read.
const EVERY: Duration = Duration::from_secs(30);

/// How long to rest while a backlog is left.
const BACKLOG: Duration = Duration::from_secs(1);

/// Reads new transcript lines until the daemon ends; the first pass runs at once.
pub fn watch(store: Store, usage: Arc<Usage>) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let rest = match usage.refresh(&store, Scope::All).await {
                Ok(true) => BACKLOG,
                Ok(false) => EVERY,
                Err(error) => {
                    tracing::warn!(%error, "reading transcripts failed");
                    EVERY
                }
            };
            tokio::time::sleep(rest).await;
        }
    })
}
