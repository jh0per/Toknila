//! What crosses the wire when an agent asks and a person answers: the permission hook's call and
//! reply, and the person's answer.

use serde::{Deserialize, Serialize};
use toknila_core::AgentRole;
use toknila_core::question::AnswerKind;

/// Body of `POST /v1/tickets/{ticket}/permission`: an agent's `PermissionRequest` hook, which
/// waits for the person's decision.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct PermissionAsk {
    /// Summary of the tool call that needs permission (the command or the file); the daemon
    /// uses the last one it was told when this is absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
}

/// What the person decided.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionDecision {
    Allow,
    Deny,
    /// Nobody decided in time: the agent shows its own dialog.
    Ask,
}

/// Answer of `POST /v1/tickets/{ticket}/permission`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PermissionReply {
    pub decision: PermissionDecision,
    /// Why it was denied, for the agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Body of `POST /v1/tickets/{ticket}/runner/answer`: a person's answer to what an agent asks.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct NewAnswer {
    /// Whose question: the ticket's `worker` (the default) or its `reviewer`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<AgentRole>,
    pub answer: AnswerKind,
    /// The text of a `text` answer; the optional comment of a `deny`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}
