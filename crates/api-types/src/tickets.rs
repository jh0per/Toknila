//! Ticket read models and the inputs that change a card.

use serde::{Deserialize, Serialize};
use toknila_app::reviews::ReviewView;
use toknila_core::machine::TicketAction;
use toknila_core::usage::UsageFigures;
use toknila_core::{Comment, Phase, Ship, Subtask, Ticket};

/// A ticket with everything attached to it.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketDetail {
    pub ticket: Ticket,
    pub chain: String,
    pub subtasks: Vec<Subtask>,
    pub comments: Vec<Comment>,
    /// Milliseconds in the current status.
    pub in_status_ms: u64,
    /// Computed from the ticket's threads: design while any is open, implement otherwise.
    pub phase: Phase,
    /// A thread of the card has a version that waits for the person's verdict (approve, or ask
    /// for changes).
    #[serde(default)]
    pub design_waiting: bool,
    /// The ticket's worktree and branch while it has them; `null` before it started, after it
    /// merged, or when its project has no repository.
    pub ship: Option<Ship>,
    /// How the card is reviewed (mode, reviewer, whose turn) and the rounds so far.
    pub review: ReviewView,
    /// What a person can do with the ticket now, computed by the state machine.
    pub allowed_actions: Vec<TicketAction>,
    /// Can the title and description still be changed? Decided by core from the status.
    pub can_edit: bool,
    /// Can the card still be moved in the chain? Decided by core from the status.
    pub can_move: bool,
    /// Place in line (1 is next) while the card waits for a free agent slot; `null` otherwise.
    pub queue_position: Option<u32>,
    /// Tokens the card's agents used and their estimated cost at API list prices; all zero
    /// before any run reported a transcript.
    #[serde(default)]
    pub usage: UsageFigures,
}

/// A ticket in a list.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketItem {
    pub ticket: Ticket,
    pub chain: String,
    pub in_status_ms: u64,
    pub needs_human: bool,
    /// Place in line (1 is next) while the card waits for a free agent slot; `null` otherwise.
    pub queue_position: Option<u32>,
}

/// Filters for `GET /v1/tickets`.
#[cfg_attr(feature = "openapi", derive(utoipa::IntoParams))]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TicketQuery {
    /// Chain id or name.
    pub chain: Option<String>,
    /// Only tickets waiting on a person (Proposed, Waiting, Blocked, Review).
    #[serde(default)]
    pub needs_human: bool,
}

/// Body of ticket actions. `text` is required by block, return and cancel.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ActionInput {
    pub text: Option<String>,
}

/// `POST /v1/tickets/{ticket}/subtasks`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewSubtask {
    pub text: String,
}

/// `PATCH /v1/tickets/{ticket}/subtasks/{position}`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PatchSubtask {
    pub done: Option<bool>,
    pub text: Option<String>,
}
