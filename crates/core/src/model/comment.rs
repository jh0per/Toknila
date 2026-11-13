use serde::{Deserialize, Serialize};

use super::{CommentId, TicketId, Timestamp};

/// Who did something: a person, an agent, or the system (graph rules, autostart).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Actor {
    Human,
    Agent,
    System,
    /// The project's assistant (the planner), answering a person's question about a story.
    Assistant,
}

/// Comment type from the template; decisions later become ADRs.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentKind {
    Instruction,
    Report,
    Decision,
    /// A person's question to the assistant about the story; never an instruction, never queued.
    Question,
}

/// Template fields of a comment. Empty fields are `None` and are omitted when rendered.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentBody {
    /// One-line gist, the heading of the comment.
    pub gist: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub what: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub how_to_verify: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alternatives: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consequences: Option<String>,
}

/// A permanent record in a ticket: an instruction from a person, an agent's report, or a decision.
/// Everything said to an agent is stored here first and delivered after.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    pub id: CommentId,
    pub ticket_id: TicketId,
    pub author: Actor,
    pub kind: CommentKind,
    pub body: CommentBody,
    pub created_at: Timestamp,
    /// When the runner delivered it to the agent; `None` while pending.
    pub delivered_at: Option<Timestamp>,
}
