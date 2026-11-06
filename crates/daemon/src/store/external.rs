//! After-commit work: what a command queued for the host (git, and the agents' processes) runs once
//! its transaction committed, off the store thread, and its outcomes are recorded in a second
//! transaction as events through the state machine. Neither git nor a process ever runs inside a transaction.

use toknila_app::{AppError, external};
use toknila_core::Actor;

use super::Store;

impl Store {
    /// Runs the queued effects on a blocking thread, then records what came of them. A failure
    /// here cannot undo the command that queued them: it is logged, and the journal tells.
    pub(super) async fn after_commit(&self, queued: Vec<external::ExternalEffect>) {
        let Some(yard) = self.yard.clone().filter(|_| !queued.is_empty()) else {
            return;
        };
        let runner = self.runner.clone();
        let outcomes = match tokio::task::spawn_blocking(move || {
            external::execute(
                external::Host {
                    yard: &*yard,
                    runner: &*runner,
                },
                queued,
            )
        })
        .await
        {
            Ok(outcomes) => outcomes,
            Err(e) => return tracing::error!(error = %e, "external effects panicked"),
        };
        let now = crate::server::now();
        let recorded = self
            .commit(Actor::System, now, move |tx| -> Result<(), AppError> {
                external::apply(tx, outcomes)
            })
            .await;
        if let Err(e) = recorded {
            tracing::error!(error = %e, "recording external outcomes failed");
        }
    }
}
