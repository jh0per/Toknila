//! Startup recovery: compare stored state with the journal, then settle every chain.
//!
//! Every command commits state and journal together, so after a kill the store is at the last
//! committed command. Recovery proves it instead of assuming it: every ticket and milestone is
//! compared with the snapshot in its newest journal event, then every chain settles once
//! (autostart that a crash interrupted between commands). Simulated tickets have no process to
//! lose, so Running stays Running. A ticket whose agent is a process lost it with the daemon:
//! it goes to Blocked(`runner_lost`) before anything settles, so a restart is never silent. A card
//! waiting for a reviewer process that is gone goes to a person.

use std::collections::HashSet;

use serde::Serialize;
use serde_json::Value;
use toknila_core::TicketStatus;

use crate::{AppError, AppResult, UnitOfWork, merges, reviews, runs, with_chain};

/// What startup recovery checked and found.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Recovery {
    pub chains: usize,
    pub tickets: usize,
    pub running: usize,
    /// Keys of the tickets whose runner the restart lost; they are Blocked now.
    pub lost: Vec<String>,
    /// Keys of the cards whose reviewer the restart lost; a person has them now.
    pub reviews_lost: Vec<String>,
    /// Keys of the cards whose merge was running; it is queued again.
    pub merges_resumed: Vec<String>,
    /// Entities whose stored state differs from their last journal snapshot. The database stays
    /// the source of truth; drift is reported, never "fixed" from the journal.
    pub drift: Vec<String>,
}

/// Checks every chain against the journal and settles it, inside one unit of work.
pub fn recover<U: UnitOfWork + ?Sized>(uow: &mut U) -> AppResult<Recovery> {
    let mut report = Recovery::default();
    // No process outlives the daemon that started it, so at startup none is live.
    let lost = runs::mark_lost(uow, &HashSet::new())?;
    report.lost = lost.iter().map(ToString::to_string).collect();
    let reviews = reviews::mark_lost(uow, &HashSet::new())?;
    report.reviews_lost = reviews.iter().map(ToString::to_string).collect();
    let resumed = merges::resume(uow)?;
    report.merges_resumed = resumed.iter().map(ToString::to_string).collect();
    let chains = uow.chains(None)?;
    report.chains = chains.len();
    for c in &chains {
        let Some(view) = uow.chain_view(c.id)? else {
            continue;
        };
        for t in &view.tickets {
            report.tickets += 1;
            if t.status == TicketStatus::Running {
                report.running += 1;
            }
            let snap = uow.last_event_payload("ticket", &t.id.to_string())?;
            if !matches(snap.as_ref(), "ticket", to_value(t)?) {
                report.drift.push(t.key.to_string());
            }
        }
        for r in &view.releases {
            let snap = uow.last_event_payload("release", &r.id.to_string())?;
            if !matches(snap.as_ref(), "release", to_value(r)?) {
                report.drift.push(format!("{} {}", c.name, r.version));
            }
        }
    }
    for c in &chains {
        with_chain(uow, c.id, |_| Ok::<_, AppError>(()))?;
    }
    Ok(report)
}

fn to_value(v: &impl Serialize) -> AppResult<Value> {
    serde_json::to_value(v).map_err(|e| AppError::internal(format!("json: {e}")))
}

fn matches(snapshot: Option<&Value>, field: &str, now: Value) -> bool {
    snapshot.and_then(|s| s.get(field)) == Some(&now)
}
