//! How many agents of a project work at once, and who starts next.
//!
//! **The limit holds on every start.** A project has `max_parallel` slots (default
//! [`DEFAULT_LIMIT`]). No agent of a story starts or resumes while the project's stories with an
//! agent are at the limit, whatever the path: a first start, a story sent back (by the check, the
//! reviewer agent, a person, a merge conflict, a failed re-check), an unblock, a start despite a
//! stale base, an answer to a paused agent, the replacement after the app restarted, the reviewer
//! after a hand-in. The state machine never starts such an agent itself: it marks the story (the
//! worker's `Ticket::agent_paused` with `resume`, `Review::reviewer_waits`), [`plan`] says when it
//! starts, and the application sends it [`start_event`].
//!
//! **Slots are stories.** A story holds one slot while an agent of it works: Running or Waiting
//! with its agent (not paused, lost or waiting to go on), or in Review once its reviewer agent's
//! turn has started. A simulated agent counts the same. A **live runner** of a story holds it
//! too, whatever the story's status: a worker still ending after its hand-in, a reviewer still
//! ending after its verdict, the agent of a story a person blocked by hand (a manual block keeps
//! its agent alive), a runner still being stopped after a merge or a cancel. A story's worker and
//! reviewer alive together hold one slot. With no live runner, Blocked, Merged, Released,
//! Cancelled and Backlog hold none. The slot is free once the process is gone: the host reports
//! every end, on purpose or by itself, and the application runs the schedule for it. The host
//! knows which runners are alive; the application passes them in and this module stays pure.
//!
//! **The line.** A story that has to go on and finds no free slot waits in line, and
//! [`wait_text`] says so ("sent back; goes on when an agent is free"). Stories already begun
//! (sent back, unblocked, answered, back after a restart, a reviewer that is due) go before the
//! stories that have not started: work in progress finishes before new work begins. Inside each
//! group the order is chain order, then the card's place in its chain's dependency order, so a
//! card never starts before the cards it waits for. A story that holds a slot through a live
//! runner of its own goes on in that slot: it never waits for itself.
//!
//! **Queued is a flag, not a status.** A card a person started while every slot was taken stays
//! Ready with `Ticket::queued` set, and a story that has begun stays in the status it went to
//! with its agent marked as not started. A new status would touch the transition table, board
//! categories, dependency rules, the database and every client for a state that is only "wanted,
//! with no agent yet"; the flags record what cannot be derived, and the place in line is computed
//! here by [`plan`], never stored.
//!
//! **Who is wanted.** A story that has not started is wanted when it is Ready with its
//! dependencies done and either `queued` or in a chain with autostart on, so the queue never
//! starts anything nobody asked for. Lowering the limit starts nothing and stops nothing: an
//! agent that is alive keeps its slot and delays the line until it leaves, even past the limit
//! (an agent's own `started` is a fact, not a request).

use crate::graph::ChainView;
use crate::machine::TicketEvent;

mod runs;
pub use runs::{LiveRun, ProjectRun, RunCard, project_runs};

use crate::{AgentRole, ReviewTurn, Ticket, TicketId, TicketStatus};

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

/// Does the card hold a slot by its state: its worker works or waits for an answer, or its
/// reviewer agent's turn has started. A card whose agent has no process (paused, lost, or waiting
/// to go on) and a reviewer that waits for a free agent hold none.
pub fn holds_slot(ticket: &Ticket) -> bool {
    match ticket.status {
        TicketStatus::Running | TicketStatus::Waiting => ticket.agent_paused.is_none(),
        TicketStatus::Review => {
            matches!(ticket.review.turn, Some(ReviewTurn::Agent { .. }))
                && !ticket.review.reviewer_waits
        }
        _ => false,
    }
}

/// Does the story hold a slot: by its state, or by a live runner of it, whatever its status
/// (Blocked by a person's hand, or still ending after a merge or a cancel). Its worker and its
/// reviewer are one slot.
pub fn holds(ticket: &Ticket, live: &[LiveRunner]) -> bool {
    holds_slot(ticket) || live.iter().any(|(id, _)| *id == ticket.id)
}

/// Has the story begun and must it go on with an agent that has no process yet: a paused or
/// lost agent whose answer arrived, a worker sent back or unblocked, a reviewer that is due.
pub fn goes_on(ticket: &Ticket) -> bool {
    ticket.agent_paused.as_ref().is_some_and(|p| p.resume) || ticket.review.reviewer_waits
}

/// The event the schedule sends a wanted card that gets an agent: `ResumeStart` for a story that
/// has begun, `Start` for one that has not.
pub fn start_event(ticket: &Ticket) -> TicketEvent {
    if goes_on(ticket) {
        TicketEvent::ResumeStart
    } else {
        TicketEvent::Start
    }
}

/// What a story waiting for a free agent says, at `place` in line (1 is next): "queued #2" for a
/// story that has not started, "sent back; goes on when an agent is free" for one that has.
pub fn wait_text(ticket: &Ticket, place: u32) -> String {
    let why = match &ticket.agent_paused {
        _ if ticket.review.reviewer_waits => "the reviewer is due",
        Some(p) if p.lost => "the app restarted",
        Some(p) => p.go_on.map_or("answered", crate::GoOn::word),
        None => return format!("queued #{place}"),
    };
    let what = if ticket.review.reviewer_waits {
        "it starts"
    } else {
        "goes on"
    };
    match place {
        1 => format!("{why}; {what} when an agent is free"),
        n => format!("{why}; {what} when an agent is free (#{n} in line)"),
    }
}

/// The state of a project's slots and what to do about it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    pub limit: u32,
    /// Slots held now: stories with an agent.
    pub used: u32,
    /// Wanted cards that start now, in order.
    pub start: Vec<TicketId>,
    /// Wanted cards that wait for a slot, first in line first.
    pub queue: Vec<TicketId>,
    /// What each card of `queue` says while it waits ([`wait_text`]), in the same order.
    pub texts: Vec<String>,
}

impl Plan {
    /// 1-based place of a card in line, if it waits.
    pub fn position(&self, ticket: TicketId) -> Option<u32> {
        let i = self.queue.iter().position(|t| *t == ticket)?;
        u32::try_from(i + 1).ok()
    }

    /// What a card that waits says, if it waits.
    pub fn text(&self, ticket: TicketId) -> Option<&str> {
        let i = self.queue.iter().position(|t| *t == ticket)?;
        self.texts.get(i).map(String::as_str)
    }
}

/// A runner process that is alive, by the ticket it works for and its role.
pub type LiveRunner = (TicketId, AgentRole);

/// The stories of `chains` that hold a slot ([`holds`]), in chain order.
pub fn working_cards(chains: &[ChainView], live: &[LiveRunner]) -> Vec<TicketId> {
    chains
        .iter()
        .flat_map(|c| &c.tickets)
        .filter(|t| holds(t, live))
        .map(|t| t.id)
        .collect()
}

/// Slots held in `chains`: one per story with an agent ([`holds`]).
pub fn slots_used(chains: &[ChainView], live: &[LiveRunner]) -> u32 {
    u32::try_from(working_cards(chains, live).len()).unwrap_or(u32::MAX)
}

/// Which wanted cards of the project's `chains` (in chain order) start now with `limit` slots,
/// given the runners that are alive, and which wait.
pub fn plan(chains: &[ChainView], limit: u32, live: &[LiveRunner]) -> Plan {
    let used = slots_used(chains, live);
    let find = |id: &TicketId| chains.iter().flat_map(|c| &c.tickets).find(|t| t.id == *id);
    let mut wanted: Vec<&Ticket> = chains
        .iter()
        .flat_map(ChainView::start_requests)
        .filter_map(|id| find(&id))
        .collect();
    // Work in progress first (the sort is stable, so each group keeps its chain order).
    wanted.sort_by_key(|t| !goes_on(t));
    let (mut start, mut queue) = (Vec::new(), Vec::new());
    let mut taken = used;
    for t in wanted {
        // A story with a live runner of its own goes on in its slot while that slot is within
        // the limit; any other takes a free one.
        let own = holds(t, live);
        if (own && taken <= limit) || (!own && taken < limit) {
            taken += u32::from(!own);
            start.push(t.id);
        } else {
            queue.push(t);
        }
    }
    Plan {
        limit,
        used,
        start,
        texts: queue
            .iter()
            .zip(1..)
            .map(|(t, n)| wait_text(t, n))
            .collect(),
        queue: queue.iter().map(|t| t.id).collect(),
    }
}

#[cfg(test)]
mod tests;
