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
            Ok(toknila_app::recovery::recover(tx)?)
        })
        .await?;
    Ok(report)
}

#[cfg(test)]
mod tests;
