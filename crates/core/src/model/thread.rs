use serde::{Deserialize, Serialize};

use super::{Actor, ThreadId, ThreadMessageId, ThreadVersionId, TicketId, Timestamp};

/// What a thread settles before the agent implements the card.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadKind {
    /// A screen design: states, layout.
    Design,
    /// A flowchart or a process.
    Flow,
    /// An architecture diagram.
    Architecture,
    /// A person's talk with the project's assistant about the card. It is born approved: it never
    /// blocks the hand-in and never counts as an approved design.
    Discussion,
    Other,
}

/// Whether the thread still needs a decision.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadState {
    Open,
    Approved,
    /// The agent took back its own thread before it had a version or a person wrote in it.
    Withdrawn,
}

/// How an artifact body is to be read.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactFormat {
    Mermaid,
    Markdown,
    Html,
}

/// Plain message, or a person's request for changes.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    Message,
    ChangesRequested,
    /// The assistant's proposed change of a card's prompt or acceptance (the text is a
    /// [`crate::discussion::EditProposal`] as JSON); it lands only when a person accepts it.
    EditProposal,
    /// A person accepted the proposal whose message id is the text.
    EditAccepted,
    /// A person rejected the proposal whose message id is the text.
    EditRejected,
}

/// Where a ticket is: settling its design while any thread is open, implementing otherwise.
/// Computed from the threads, never stored.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Design,
    Implement,
}

/// A discussion on one ticket that ends in a person's approval.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Thread {
    pub id: ThreadId,
    pub ticket_id: TicketId,
    /// 1-based, per ticket; how people refer to it.
    pub number: u32,
    pub kind: ThreadKind,
    pub title: String,
    pub state: ThreadState,
    pub opened_by: Actor,
    pub created_at: Timestamp,
    /// Version number the approval pinned; `None` while open, or when approved without one.
    pub approved_version: Option<u32>,
    /// When a person approved it; `None` while open.
    pub approved_at: Option<Timestamp>,
}

/// One message in a thread.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadMessage {
    pub id: ThreadMessageId,
    pub thread_id: ThreadId,
    pub author: Actor,
    pub kind: MessageKind,
    pub text: String,
    pub created_at: Timestamp,
}

/// An immutable, numbered version of the thread's artifact.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadVersion {
    pub id: ThreadVersionId,
    pub thread_id: ThreadId,
    /// 1-based, per thread.
    pub number: u32,
    pub author: Actor,
    pub format: ArtifactFormat,
    pub body: String,
    pub created_at: Timestamp,
}
