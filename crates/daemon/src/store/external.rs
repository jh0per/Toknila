//! After-commit work: what a command queued for the host (git, and the agents' processes) runs once
//! its transaction committed, off the store thread, and its outcomes are recorded in a second
//! transaction as events through the state machine. Neither git nor a process ever runs inside a transaction.

use toknila_app::{AppError, external};
use toknila_core::Actor;

use super::Store;

/// Rounds of "run effects, record outcomes" one command may cause. An outcome can free a slot
/// (a failed ship blocks its card), which starts the next card, which queues new effects.
const MAX_ROUNDS: usize = 16;

impl Store {
    /// Runs the queued effects on a blocking thread, then records what came of them. A failure
    /// here cannot undo the command that queued them: it is logged, and the journal tells.
    /// Recording can itself queue work (a freed slot starts the next card); that runs too.
    pub(super) async fn after_commit(&self, queued: Vec<external::ExternalEffect>) {
        let Some(yard) = self.yard.clone() else {
            return;
        };
        let mut queued = queued;
        for _ in 0..MAX_ROUNDS {
            if queued.is_empty() {
                return;
            }
            let runner = self.runner.clone();
            let yard = yard.clone();
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
                .commit_with_effects(Actor::System, now, move |tx| -> Result<(), AppError> {
                    external::apply(tx, outcomes)
                })
                .await;
            queued = match recorded {
                Ok(((), _, more)) => more,
                Err(e) => return tracing::error!(error = %e, "recording external outcomes failed"),
            };
        }
        tracing::error!("external effects kept queueing more; stopped");
    }
}
