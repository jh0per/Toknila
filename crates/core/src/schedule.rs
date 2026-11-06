//! How many agents of a project work at once, and who starts next.
//!
//! **Slots.** A project has `max_parallel` slots (default [`DEFAULT_LIMIT`]). A card holds one
//! while it is Running or Waiting (its agent is alive; a simulated agent counts the same).
//! Review, Blocked, Merged, Released, Cancelled and Backlog hold none: the agent is done, stopped
//! or has not begun.
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
use crate::{Ticket, TicketId, TicketStatus};

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

/// Which wanted cards of the project's `chains` (in chain order) start now with `limit` slots.
pub fn plan(chains: &[ChainView], limit: u32) -> Plan {
    let used = chains
        .iter()
        .flat_map(|c| &c.tickets)
        .filter(|t| holds_slot(t))
        .count();
    let used = u32::try_from(used).unwrap_or(u32::MAX);
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
