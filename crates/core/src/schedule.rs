//! How many agents of a project work at once, and who starts next.
//!
//! **Slots.** A project has `max_parallel` slots (default [`DEFAULT_LIMIT`]). A card holds one
//! while it is Running or Waiting (its agent is alive; a simulated agent counts the same).
//! Review, Blocked, Merged, Released, Cancelled and Backlog hold none by their status alone: the
//! agent is done, stopped or has not begun. A **live runner** holds one whatever the status: a
//! worker kept alive through review, and the reviewer next to it, each take a slot. The host knows
//! which runners are alive; the application passes them in and this module stays pure.
//!
//! **Queued is a flag, not a status.** A card a person started while every slot was taken stays
//! Ready with `Ticket::queued` set. A new status would touch the transition table, board
//! categories, dependency rules, the database and every client for a state that is only "Ready,
//! and wanted"; the flag records the one thing that cannot be derived (a person asked), and the
//! place in line is computed here by [`plan`], never stored.
//!
//! **Who starts.** [`plan`] is pure: given the project's chains (in chain order) and the limit it
//! says which wanted cards start now and which wait, in order. A card is wanted when it is Ready
//! with its dependencies done and either `queued` or in a chain with autostart on. Chains with
//! autostart off therefore only run cards a person started; the queue never starts anything
//! nobody asked for. The order is chain order, then the card's place in its chain's dependency
//! order, so a card never starts before the cards it waits for. Lowering the limit starts
//! nothing and stops nothing: running agents keep their slots until they leave Running/Waiting.

use crate::graph::ChainView;
use std::collections::HashSet;

mod runs;
pub use runs::{LiveRun, ProjectRun, RunCard, project_runs};

use crate::{AgentRole, Ticket, TicketId, TicketStatus};

/// Slots of a project that does not say.
pub const DEFAULT_LIMIT: u32 = 2;
/// Most slots a project may set.
pub const MAX_LIMIT: u32 = 64;

/// Why a limit was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("parallel agents must be between 1 and {MAX_LIMIT}, not {0}")]
pub struct LimitError(pub u32);

/// A valid limit, or why not.
pub fn check_limit(limit: u32) -> Result<u32, LimitError> {
    if (1..=MAX_LIMIT).contains(&limit) {
        Ok(limit)
    } else {
        Err(LimitError(limit))
    }
}

/// Does the card hold a slot: its agent is working or waiting for an answer.
pub fn holds_slot(ticket: &Ticket) -> bool {
    matches!(ticket.status, TicketStatus::Running | TicketStatus::Waiting)
}

/// The state of a project's slots and what to do about it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    pub limit: u32,
    /// Slots held now.
    pub used: u32,
    /// Wanted cards that start now, in order.
    pub start: Vec<TicketId>,
    /// Wanted cards that wait for a slot, first in line first.
    pub queue: Vec<TicketId>,
}

impl Plan {
    /// 1-based place of a card in line, if it waits.
    pub fn position(&self, ticket: TicketId) -> Option<u32> {
        let i = self.queue.iter().position(|t| *t == ticket)?;
        u32::try_from(i + 1).ok()
    }
}

/// A runner process that is alive, by the ticket it works for and its role.
pub type LiveRunner = (TicketId, AgentRole);

/// The cards of `chains` that hold a slot: each working or waiting card (its worker), and each
/// card with a live runner that is working or in review, in chain order.
pub fn working_cards(chains: &[ChainView], live: &[LiveRunner]) -> Vec<TicketId> {
    chains
        .iter()
        .flat_map(|c| &c.tickets)
        .filter(|t| {
            holds_slot(t)
                || (matches!(t.status, TicketStatus::Review)
                    && live.iter().any(|(id, _)| *id == t.id))
        })
        .map(|t| t.id)
        .collect()
}

/// Slots held in `chains`: each working or waiting card (its worker), and each live runner of a
/// card of these chains, counted once.
pub fn slots_used(chains: &[ChainView], live: &[LiveRunner]) -> u32 {
    let tickets = || chains.iter().flat_map(|c| &c.tickets);
    let mut held: HashSet<LiveRunner> = tickets()
        .filter(|t| holds_slot(t))
        .map(|t| (t.id, AgentRole::Worker))
        .collect();
    // A runner counts while its card is working or in review. A card that was merged,
    // cancelled or blocked has its runners stopped right after the command that moved it, so
    // they already free their slots for the cards this command starts.
    held.extend(
        live.iter()
            .filter(|(id, _)| {
                tickets().any(|t| {
                    t.id == *id
                        && matches!(
                            t.status,
                            TicketStatus::Running | TicketStatus::Waiting | TicketStatus::Review
                        )
                })
            })
            .copied(),
    );
    u32::try_from(held.len()).unwrap_or(u32::MAX)
}

/// Which wanted cards of the project's `chains` (in chain order) start now with `limit` slots,
/// given the runners that are alive.
pub fn plan(chains: &[ChainView], limit: u32, live: &[LiveRunner]) -> Plan {
    let used = slots_used(chains, live);
    let mut wanted: Vec<TicketId> = chains.iter().flat_map(ChainView::start_requests).collect();
    let free = usize::try_from(limit.saturating_sub(used)).unwrap_or(usize::MAX);
    let queue = wanted.split_off(free.min(wanted.len()));
    Plan {
        limit,
        used,
        start: wanted,
        queue,
    }
}

#[cfg(test)]
mod tests;
