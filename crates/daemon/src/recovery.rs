//! Startup recovery after an unclean stop (crash, `kill -9`, power loss).
//!
//! Every command commits state and journal in one SQLite transaction, so a kill leaves the
//! database at the last committed command. On start the daemon proves that instead of assuming
//! it: SQLite's integrity check, then every ticket and milestone is compared with the snapshot in
//! its newest journal event, then every chain settles once (autostart that a crash interrupted
//! between commands). Simulated tickets have no process to lose, so Running stays Running.

use serde::Serialize;
use serde_json::Value;
use toknila_core::Actor;

use crate::api::commands::with_chain;
use crate::api::error::ApiError;
use crate::server::now;
use crate::store::{Store, StoreError};

/// What startup recovery checked and found.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Recovery {
    pub chains: usize,
    pub tickets: usize,
    pub running: usize,
    /// Entities whose stored state differs from their last journal snapshot. The database stays
    /// the source of truth; drift is reported, never "fixed" from the journal.
    pub drift: Vec<String>,
}

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
            let mut report = Recovery::default();
            let chains = tx.repo().chains(None)?;
            report.chains = chains.len();
            for c in &chains {
                let Some(view) = tx.repo().chain_view(c.id)? else {
                    continue;
                };
                for t in &view.tickets {
                    report.tickets += 1;
                    if t.status == toknila_core::TicketStatus::Running {
                        report.running += 1;
                    }
                    let snap = tx.repo().last_event_payload("ticket", &t.id.to_string())?;
                    if !matches(snap.as_ref(), "ticket", to_value(t)?) {
                        report.drift.push(t.key.to_string());
                    }
                }
                for r in &view.releases {
                    let snap = tx.repo().last_event_payload("release", &r.id.to_string())?;
                    if !matches(snap.as_ref(), "release", to_value(r)?) {
                        report.drift.push(format!("{} {}", c.name, r.version));
                    }
                }
            }
            for c in &chains {
                with_chain(tx, c.id, |_, _| Ok(()))?;
            }
            Ok(report)
        })
        .await?;
    Ok(report)
}

fn to_value(v: &impl Serialize) -> Result<Value, StoreError> {
    Ok(serde_json::to_value(v)?)
}

fn matches(snapshot: Option<&Value>, field: &str, now: Value) -> bool {
    snapshot.and_then(|s| s.get(field)) == Some(&now)
}

#[cfg(test)]
mod tests;
