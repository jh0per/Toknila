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
    /// The 0-based index of the choice the person pressed, for an `ask_human` question that
    /// offered choices; the agent is told the choice's text. Use with `answer: text`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<u32>,
    /// With `allow` on a worker's Bash prompt: also remember a rule for the command in the
    /// project ("Allow always"), so later runs and prompts of running agents it covers are allowed
    /// without asking. Ignored for any other answer or tool.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub always: bool,
}

/// Answer of `POST /v1/tickets/{ticket}/runner/answer` when the person chose "Allow always":
/// what was added to the project, or why nothing was.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AnswerOutcome {
    /// The rule now remembered in the project (`Bash(cargo test *)`); absent when none was made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_added: Option<String>,
    /// One plain sentence saying what happened.
    pub note: String,
}
