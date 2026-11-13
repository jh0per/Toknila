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

/// A Waiting card whose agent's process was ended after a grace, so the card holds no slot while
/// it waits for a person. It keeps what the run needs to come back: Claude Code's session id.
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
}
