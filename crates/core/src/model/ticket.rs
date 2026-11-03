use serde::{Deserialize, Serialize};

use super::{
    Actor, ChainId, ReviewMode, StatusReason, SubtaskId, TicketId, TicketKey, TicketStatus,
    Timestamp,
};

/// One unit of work for one ship.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ticket {
    pub id: TicketId,
    pub chain_id: ChainId,
    pub key: TicketKey,
    pub title: String,
    /// The task given to the agent; the only source of what to do.
    pub prompt: String,
    /// Acceptance criteria, one per entry.
    pub acceptance: Vec<String>,
    pub status: TicketStatus,
    /// Present exactly when the status carries a reason (Waiting, Blocked, Cancelled).
    pub status_reason: Option<StatusReason>,
    pub status_since: Timestamp,
    /// Paused tickets stay in Backlog and are skipped by autostart.
    pub paused: bool,
    /// Agent profile name; `None` means the project default.
    pub profile: Option<String>,
    /// `None` means the chain's, then the project's review mode.
    pub review_mode: Option<ReviewMode>,
    /// Ship branch `ship/<chain>/<slug>`; empty until phase 1 creates ships.
    pub branch: Option<String>,
    /// Copy depot secrets onto the ship at start.
    pub needs_secrets: bool,
    pub tokens_used: u64,
}

/// A checklist item inside a ticket. Subtasks have no runners of their own.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subtask {
    pub id: SubtaskId,
    pub ticket_id: TicketId,
    /// 1-based position in the checklist.
    pub position: u32,
    pub text: String,
    pub done: bool,
    /// Who ticked it, when done.
    pub done_by: Option<Actor>,
}
