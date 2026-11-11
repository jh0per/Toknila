//! After-commit work: what a command queued for the host (git, and the agents' processes) runs once
//! its transaction committed, off the store thread, and its outcomes are recorded in a second
//! transaction as events through the state machine. Neither git nor a process ever runs inside a transaction.

use std::sync::Arc;

use toknila_app::ships::{self, GuardError};
use toknila_app::{
    AppError, MergeError, MergeOutcome, Merger, ShipError, Shipyard,
    external::{self, ExternalEffect, ExternalOutcome},
};
use toknila_core::guard::ShipState;
use toknila_core::{Actor, MergeSpec, Ship, ShipLook, ShipSpec, TicketId};

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
                // Checks that are left undone also leave the line they joined at the commit.
                for e in &queued {
                    if let ExternalEffect::RunCheck { ticket, .. } = e {
                        self.line.leave(*ticket);
                    }
                }
                queued.retain(|e| matches!(e, ExternalEffect::Merge { .. }));
            }
            if queued.is_empty() {
                return;
            }
            self.log_checks_starting(&queued).await;
            // The checks joined the line when their command committed (`commit_with_effects`).
            let joined: Vec<TicketId> = queued
                .iter()
                .filter_map(|e| match e {
                    ExternalEffect::RunCheck { ticket, .. } => Some(*ticket),
                    _ => None,
                })
                .collect();
            let runner = self.runner.clone();
            let merger = self.merger.clone();
            let checker = self.checker.clone();
            let line = self.line.clone();
            let yard = yard.clone();
            let outcomes = tokio::task::spawn_blocking(move || {
                external::execute(
                    external::Host {
                        yard: &*yard,
                        merger: &*merger,
                        runner: &*runner,
                        checker: &*checker,
                        gate: &*line,
                    },
                    queued,
                )
            })
            .await;
            for t in &joined {
                self.line.leave(*t);
            }
            let outcomes = match outcomes {
                Ok(outcomes) => outcomes,
                Err(e) => return tracing::error!(error = %e, "external effects panicked"),
            };
            let mut outcomes = outcomes;
            if self.tasks.closing() {
                // The daemon is stopping: a check that was killed with it did not end, and its
                // card is handed to a person at the next start.
                outcomes.retain(|o| !matches!(o, ExternalOutcome::CheckFinished { .. }));
            }
            self.log_checks_ended(&outcomes).await;
            if outcomes.is_empty() {
                return;
            }
            let now = crate::server::now();
            let recorded = self
                .commit_with_effects(
                    Actor::System,
                    now,
                    false,
                    move |tx| -> Result<(), AppError> { external::apply(tx, outcomes) },
                )
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
    /// The key of a ticket for the log.
    async fn key_of(&self, ticket: TicketId) -> String {
        match self.read(move |r| r.ticket(ticket)).await {
            Ok(Some(t)) => t.key.to_string(),
            _ => ticket.to_string(),
        }
    }

    async fn log_checks_starting(&self, queued: &[ExternalEffect]) {
        for e in queued {
            if let ExternalEffect::RunCheck { ticket, spec, sync } = e {
                let key = self.key_of(*ticket).await;
                match spec {
                    Some(s) => {
                        tracing::info!(ticket = %key, command = %s.command, dir = %s.dir, "check starting");
                    }
                    None if sync.is_some() => {
                        tracing::info!(ticket = %key, "check only brings the ship up to its base");
                    }
                    None => tracing::info!(ticket = %key, "check has nothing to run"),
                }
            }
        }
    }

    async fn log_checks_ended(&self, outcomes: &[ExternalOutcome]) {
        for o in outcomes {
            if let ExternalOutcome::CheckFinished { ticket, result, .. } = o {
                let key = self.key_of(*ticket).await;
                let status = match result {
                    Ok(r) if r.timed_out => "timed out".to_owned(),
                    Ok(r) if r.success => "passed".to_owned(),
                    Ok(r) => format!("failed (code {:?})", r.code),
                    Err(e) => format!("could not run: {e}"),
                };
                tracing::info!(ticket = %key, status = %status, "check ended, recording the result");
            }
        }
    }

    /// The shipyard the store's commands make ships in; `None` when tickets run without ships.
    pub fn shipyard(&self) -> Option<Yard> {
        self.yard.clone()
    }

    /// Looks at a ship's worktree on a blocking thread, outside any transaction, and judges it
    /// (`finished` is refused when the work is not committed). A store without a shipyard has no
    /// ships, so there is nothing to refuse.
    pub async fn guard_ship(&self, look: ShipLook) -> Result<ShipState, GuardError> {
        let Some(yard) = self.yard.clone() else {
            return Ok(ShipState::default());
        };
        let judged = look.clone();
        tokio::task::spawn_blocking(move || ships::guarded(&*yard, &judged))
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
