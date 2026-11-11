//! Startup recovery after an unclean stop (crash, `kill -9`, power loss).
//!
//! SQLite's integrity check runs first; then [`toknila_app::recovery::recover`] compares every
//! ticket and milestone with its journal snapshot and settles every chain, all in one
//! transaction.

use toknila_core::Actor;

pub use toknila_app::recovery::Recovery;

use crate::http::error::ApiError;
use crate::server::now;
use crate::store::{Store, StoreError};

/// Checks the store after start and settles every chain, all in one transaction.
pub async fn recover(store: &Store) -> Result<Recovery, ApiError> {
    let (report, _) = store
        .write(Actor::System, now(), |tx| -> Result<Recovery, ApiError> {
            let check = tx.repo().integrity_check()?;
            if check != "ok" {
                return Err(ApiError::from(StoreError::Corrupt(format!(
                    "database integrity check: {check}"
                ))));
            }
            interrupt_release_runs(tx)?;
            Ok(toknila_app::recovery::recover(tx)?)
        })
        .await?;
    Ok(report)
}

/// A release run that was going when the daemon stopped is marked interrupted at the step it was
/// in: its steps are in the journal, and a person can run it again from there.
fn interrupt_release_runs(tx: &mut crate::store::WriteTx<'_>) -> Result<(), StoreError> {
    use crate::store::release_runs::{Entry, interrupted_at};
    for id in tx.repo().release_run_ids()? {
        let Some(run) = tx.repo().release_run(id)? else {
            continue;
        };
        if run.state == toknila_api_types::RunState::Running {
            let step = interrupted_at(&run);
            tx.journal_release_run(id, &Entry::RunInterrupted { step })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
