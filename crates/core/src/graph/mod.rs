//! The chain graph: tickets and release milestones joined by dependency edges.
//!
//! The graph decides what may start: [`ChainView::next_actions`] lists what should happen now
//! (a Backlog ticket became Ready, a Ready ticket autostarts, an auto milestone is cut), and
//! [`ChainView::settle`] applies those actions until nothing changes. Chain status, progress and
//! "where it stopped" are computed by [`ChainView::summary`] and never stored.
//!
//! Releases follow gitflow: a milestone is `planned`, then `cut` (a `release/<version>` branch
//! leaves `develop`), then `released`. Tickets after a milestone wait only for the cut; an
//! `after_release` edge makes them wait for the release itself.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::machine::{self, Effect, TicketEvent, TransitionError};
use crate::{
    BlockedReason, Chain, Edge, EdgeKind, NodeRef, Release, ReleaseChannel, ReleaseId,
    ReleaseState, StartMode, StatusReason, Ticket, TicketId, TicketKey, TicketStatus, Timestamp,
};

/// Why the graph or a graph operation was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GraphError {
    #[error("edge refers to a node that is not in this chain: {0:?}")]
    UnknownNode(NodeRef),
    #[error("a node cannot depend on itself: {0:?}")]
    SelfLoop(NodeRef),
    #[error("duplicate edge {from:?} -> {to:?}")]
    DuplicateEdge { from: NodeRef, to: NodeRef },
    #[error("after_release edges must start at a release milestone, not {0:?}")]
    AfterReleaseFromTicket(NodeRef),
    #[error("dependency cycle: {}", format_cycle(.0))]
    Cycle(Vec<String>),
    #[error("unknown ticket {0}")]
    UnknownTicket(TicketId),
    #[error("unknown release {0}")]
    UnknownRelease(ReleaseId),
    #[error("release {version} is {state:?}; cannot {action}")]
    ReleaseState {
        version: String,
        state: ReleaseState,
        action: &'static str,
    },
    #[error("release {version} still waits for: {}", .waiting_for.join(", "))]
    ReleaseNotReady {
        version: String,
        waiting_for: Vec<String>,
    },
    #[error(transparent)]
    Transition(#[from] TransitionError),
    #[error("graph did not settle after {0} steps")]
    NoFixpoint(usize),
    #[error("{0} is a proposed card; approve or reject it first")]
    StillProposed(String),
    #[error("{node} is already {state}; a card can only go before work that has not started")]
    BeforeStarted { node: String, state: String },
    #[error("{node} is already {state}; what it waits for can no longer change")]
    DependencyLocked { node: String, state: String },
    #[error("{0} is not a proposed card")]
    NotProposed(String),
}

fn format_cycle(path: &[String]) -> String {
    path.join(" -> ")
}

/// Something the graph wants done.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    /// Apply this event to the ticket through the state machine.
    Ticket {
        ticket: TicketId,
        event: TicketEvent,
    },
    /// Cut the milestone (auto mode, everything before it is done).
    CutRelease { release: ReleaseId },
}

/// One applied step, for the event log.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Applied {
    pub action: Action,
    pub effects: Vec<Effect>,
}

/// Everything the graph rules need about one chain, as plain data.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainView {
    pub chain: Chain,
    pub tickets: Vec<Ticket>,
    pub releases: Vec<Release>,
    pub edges: Vec<Edge>,
    /// Which tickets went into which release; filled when a milestone is cut.
    pub release_tickets: Vec<(ReleaseId, TicketId)>,
}

/// Computed chain status (never stored). Proposed wildcard cards are not part of the plan: they
/// do not count in `total` or `done`, but a pending decision makes the chain need a human.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChainStatus {
    /// No tickets yet.
    Empty,
    /// Nothing started yet.
    NotStarted,
    /// Some ticket is Blocked.
    Blocked,
    /// Some ticket waits on a person (Waiting, Review, or a wildcard card to approve).
    NeedsHuman,
    /// Agents are working.
    Running,
    /// Nothing runs and nothing waits on a person, but work remains (paused or waiting on a
    /// manual milestone).
    Idle,
    /// Every ticket is done, every milestone released, and no card waits for approval.
    Done,
}

/// Where the chain stopped: the first ticket, in graph order, that waits on a person (Waiting or
/// Blocked first, then Review, then a card to approve).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoppedAt {
    pub ticket: TicketKey,
    pub status: TicketStatus,
    pub reason: Option<StatusReason>,
    pub since: Timestamp,
    pub waiting_ms: u64,
}

/// The next milestone on the graph.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NextRelease {
    pub release: ReleaseId,
    pub version: String,
    pub channel: ReleaseChannel,
    pub state: ReleaseState,
}

/// What a product person sees at a glance.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainSummary {
    pub status: ChainStatus,
    /// Tickets in the plan: wildcard cards count only once approved.
    pub total: usize,
    /// Merged, Released or Cancelled (Definition of Done: in the work branch), of `total`.
    pub done: usize,
    pub released: usize,
    pub stopped_at: Option<StoppedAt>,
    pub next_release: Option<NextRelease>,
}

mod journal;
mod moves;
mod order;
mod release;
mod settle;
mod summary;
mod validate;
mod wildcard;

pub use journal::Settled;
pub use moves::{MovePlan, Place};

impl ChainView {
    /// Human label of a node: ticket key or `release <version>`.
    pub fn label(&self, node: NodeRef) -> String {
        match node {
            NodeRef::Ticket(id) => self
                .ticket(id)
                .map(|t| t.key.to_string())
                .unwrap_or_else(|| id.to_string()),
            NodeRef::Release(id) => self
                .release(id)
                .map(|r| format!("release {}", r.version))
                .unwrap_or_else(|| id.to_string()),
        }
    }

    fn nodes(&self) -> BTreeSet<NodeRef> {
        let tickets = self.tickets.iter().map(|t| NodeRef::Ticket(t.id));
        let releases = self.releases.iter().map(|r| NodeRef::Release(r.id));
        tickets.chain(releases).collect()
    }

    fn ticket(&self, id: TicketId) -> Option<&Ticket> {
        self.tickets.iter().find(|t| t.id == id)
    }

    fn release(&self, id: ReleaseId) -> Option<&Release> {
        self.releases.iter().find(|r| r.id == id)
    }

    /// Stable order for nodes: tickets by number, then milestones by version.
    fn sort_key(&self, n: NodeRef) -> (u8, u32, String) {
        match n {
            NodeRef::Ticket(id) => (
                0,
                self.ticket(id).map_or(0, |t| t.key.number()),
                String::new(),
            ),
            NodeRef::Release(id) => (
                1,
                0,
                self.release(id)
                    .map(|r| r.version.clone())
                    .unwrap_or_default(),
            ),
        }
    }

    fn successors(&self) -> BTreeMap<NodeRef, Vec<NodeRef>> {
        let mut out: BTreeMap<NodeRef, Vec<NodeRef>> = BTreeMap::new();
        for e in &self.edges {
            out.entry(e.from).or_default().push(e.to);
        }
        out
    }
}

#[cfg(test)]
mod tests;
