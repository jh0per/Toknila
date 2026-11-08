//! Threads of a card with their messages and versions.

use serde::{Deserialize, Serialize};
use toknila_core::{Phase, Thread, ThreadMessage, ThreadVersion};

/// A thread with its messages and artifact versions.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadDetail {
    pub thread: Thread,
    /// Oldest first.
    pub messages: Vec<ThreadMessage>,
    /// By number; immutable.
    pub versions: Vec<ThreadVersion>,
    /// The thread is open and a person's request for changes has no newer version yet.
    pub changes_requested: bool,
}

/// `GET /v1/tickets/{ticket}/threads`: every thread of a ticket and the ticket's phase.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketThreads {
    /// The ticket's key.
    pub ticket: String,
    pub phase: Phase,
    pub threads: Vec<ThreadDetail>,
}
