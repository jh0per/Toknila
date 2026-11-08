//! After-commit work: what a command queued for the host (git, and the agents' processes) runs once
//! its transaction committed, off the store thread, and its outcomes are recorded in a second
//! transaction as events through the state machine. Neither git nor a process ever runs inside a transaction.

use std::sync::Arc;

use toknila_app::ships::{self, GuardError};
use toknila_app::{
    AppError, MergeError, MergeOutcome, Merger, ShipError, Shipyard,
    external::{self, ExternalEffect},
};
use toknila_core::{Actor, MergeSpec, Ship, ShipLook, ShipSpec};

use super::{Store, Yard};

/// Rounds of "run effects, record outcomes" one command may cause. An outcome can free a slot
/// (a failed ship blocks its card), which starts the next card, which queues new effects.
const MAX_ROUNDS: usize = 16;

impl Store {
    /// Runs the queued effects on a blocking thread, then records what came of them. A failure
    /// here cannot undo the command that queued them: it is logged, and the journal tells.
    /// Recording can itself queue work (a freed slot starts the next card); that runs too.
    pub(super) async fn after_commit(&self, queued: Vec<external::ExternalEffect>) {
        // Without a shipyard no card has a ship, so a merge has nothing to do and answers at
        // once; everything else the store leaves undone, as before.
        let yard: Yard = self.yard.clone().unwrap_or_else(|| Arc::new(NoYard));
        let only_merges = self.yard.is_none();
        let mut queued = queued;
        for _ in 0..MAX_ROUNDS {
            if only_merges {
                queued.retain(|e| matches!(e, ExternalEffect::Merge { .. }));
            }
            if queued.is_empty() {
                return;
            }
            let runner = self.runner.clone();
            let merger = self.merger.clone();
            let yard = yard.clone();
            let outcomes = match tokio::task::spawn_blocking(move || {
                external::execute(
                    external::Host {
                        yard: &*yard,
                        merger: &*merger,
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

impl Store {
    /// The shipyard the store's commands make ships in; `None` when tickets run without ships.
    pub fn shipyard(&self) -> Option<Yard> {
        self.yard.clone()
    }

    /// Looks at a ship's worktree on a blocking thread, outside any transaction, and judges it
    /// (`finished` is refused when the work is not committed). A store without a shipyard has no
    /// ships, so there is nothing to refuse.
    pub async fn guard_ship(&self, look: ShipLook) -> Result<(), GuardError> {
        let Some(yard) = self.yard.clone() else {
            return Ok(());
        };
        let judged = look.clone();
        tokio::task::spawn_blocking(move || ships::guard(&*yard, &judged))
            .await
            .unwrap_or_else(|e| {
                Err(GuardError::Ship(ShipError::Io(format!(
                    "guard panicked: {e}"
                ))))
            })
    }
}

/// The shipyard of a store that has none: it is never asked to make or remove anything.
#[derive(Debug)]
struct NoYard;

impl Shipyard for NoYard {
    fn prepare(&self, _spec: &ShipSpec) -> Result<String, ShipError> {
        Err(ShipError::Io("no shipyard is configured".to_owned()))
    }

    fn remove(&self, _repo: &str, _ship: &Ship) -> Result<(), ShipError> {
        Ok(())
    }

    fn retire(
        &self,
        _repo: &str,
        _ship: &Ship,
        _start: &str,
    ) -> Result<toknila_app::BranchFate, ShipError> {
        Ok(toknila_app::BranchFate::Deleted)
    }
}

/// The merger of a store that has none.
#[derive(Debug)]
pub(super) struct NoMerger;

impl Merger for NoMerger {
    fn merge(&self, _spec: &MergeSpec) -> Result<MergeOutcome, MergeError> {
        Err(MergeError::Io("no merger is configured".to_owned()))
    }
}
