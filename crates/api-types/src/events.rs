//! The journal, what a ticket's agent reports, and the error body of every failure.

use serde::{Deserialize, Serialize};
use toknila_core::{Actor, Timestamp};

/// Query of `GET /v1/events`.
#[cfg_attr(feature = "openapi", derive(utoipa::IntoParams))]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct EventQuery {
    /// Return events with seq greater than this.
    #[serde(default)]
    pub since: i64,
    /// At most this many (default 1000).
    pub limit: Option<usize>,
}

/// One journal entry. `seq` is increasing; clients resume from it.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventRecord {
    pub seq: i64,
    pub at: Timestamp,
    pub actor: Actor,
    pub entity: String,
    pub entity_id: String,
    pub kind: String,
    #[cfg_attr(feature = "openapi", schema(value_type = Object))]
    pub payload: serde_json::Value,
}

/// Body of `POST /v1/tickets/{ticket}/activity`: a sign of life from an agent's hook.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NewActivity {
    /// What fired, for example `hook:stop`.
    pub signal: String,
    /// Claude Code's session id, from the hook's input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// The session's transcript file, from the hook's input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<String>,
    /// A `PreToolUse` hook: the summary of the tool call that is about to run (the command or the
    /// file, at most 300 characters), kept to say what a later permission prompt is about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// A permission prompt notification: its message. The ticket's worker then waits for the
    /// person; a reviewer's prompt is listed in the inbox.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

impl NewActivity {
    /// A bare sign of life.
    pub fn signal(signal: impl Into<String>) -> Self {
        Self {
            signal: signal.into(),
            session_id: None,
            transcript_path: None,
            tool: None,
            prompt: None,
        }
    }
}

/// Answer of `POST /v1/tickets/{ticket}/instructions/claim`: what a person told the agent while it
/// worked, now handed over and marked delivered.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct InstructionClaim {
    /// How many instructions were handed over; 0 when nothing was pending.
    pub count: usize,
    /// The instructions as one text for the agent, in the comment template; empty when none.
    pub reason: String,
}

/// Error body of every failed request.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    /// Human-readable message.
    pub error: String,
    /// Stable machine code: not_found, bad_request, conflict, ambiguous, internal, unauthorized.
    pub code: String,
    /// The input the error belongs to (`repo_path`, `work_branch`, `default_profile`), when it
    /// is about one field; a form shows the message next to that field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}
