use serde::{Deserialize, Serialize};

use super::{StatusReason, Ticket, Timestamp, WaitingReason};

impl Ticket {
    /// Words for what a Waiting card waits for, for the run log, `tok show` and the card.
    pub fn waiting_for(&self) -> String {
        match &self.status_reason {
            Some(StatusReason::Waiting(WaitingReason::Question { text, .. })) => {
                format!("your answer to: {text}")
            }
            Some(StatusReason::Waiting(WaitingReason::Permission { text, .. })) => {
                format!("your permission: {text}")
            }
            Some(StatusReason::Waiting(WaitingReason::Silent)) => {
                "you: the agent went quiet".to_owned()
            }
            Some(StatusReason::Waiting(WaitingReason::Screen)) => {
                "you: Claude Code asks on its screen".to_owned()
            }
            _ => "a person".to_owned(),
        }
    }
}

/// Why a story that has begun goes on with a new run of its agent once an agent is free.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "why", rename_all = "snake_case")]
pub enum GoOn {
    /// Sent back to its worker: by the check, the reviewer agent, a person, or a merge conflict.
    SentBack,
    /// A person unblocked it; `stale` is their decision about a base branch that is behind.
    Unblocked {
        #[serde(default, skip_serializing_if = "is_refuse")]
        stale: super::StalePolicy,
    },
}

fn is_refuse(p: &super::StalePolicy) -> bool {
    *p == super::StalePolicy::Refuse
}

impl GoOn {
    /// What the story says while it waits: "sent back", "unblocked".
    pub fn word(self) -> &'static str {
        match self {
            Self::SentBack => "sent back",
            Self::Unblocked { .. } => "unblocked",
        }
    }
}

/// A card whose agent has no process and that comes back through the schedule: a Waiting card
/// whose agent was ended after a grace (it holds no slot while it waits for a person), a card
/// whose agent the app lost, or one that has to go on (`go_on`) and waits for a free agent. It
/// keeps what the run needs to come back: Claude Code's session id.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PausedAgent {
    /// The agent's own session, for the profile's resume form; `None` when the run never said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// When the process was ended.
    pub since: Timestamp,
    /// The answer arrived: the card starts again as soon as a slot is free.
    #[serde(default)]
    pub resume: bool,
    /// The process was lost (the app restarted), not ended on purpose: the run that replaces it
    /// is told so, and a second loss soon after blocks the card.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub lost: bool,
    /// The story went on (sent back, unblocked) while every agent was busy: its worker starts a
    /// new run, as a start does, when one is free. `None` for a paused or lost agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub go_on: Option<GoOn>,
}
