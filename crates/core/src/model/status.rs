use serde::{Deserialize, Serialize};

use super::TicketKey;

/// Ticket status. Transitions between them are owned by the state machine (P0-04).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TicketStatus {
    /// Exists, but has open dependencies or is paused.
    Backlog,
    /// All dependencies satisfied; autostart picks it up.
    Ready,
    /// A runner is alive and working.
    Running,
    /// The agent asked something, wants a permission, or went silent.
    Waiting,
    /// Cannot proceed; always carries a reason.
    Blocked,
    /// The agent finished; a diff waits for review.
    Review,
    /// Merged into the work branch. This is "Done".
    Merged,
    /// Shipped as part of a released milestone.
    Released,
    /// Won't do; kept for history instead of deleting.
    Cancelled,
}

/// Board column a status belongs to, as in Jira.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusCategory {
    ToDo,
    InProgress,
    InReview,
    Done,
}

impl TicketStatus {
    /// Every status, in lifecycle order.
    pub const ALL: [TicketStatus; 9] = [
        Self::Backlog,
        Self::Ready,
        Self::Running,
        Self::Waiting,
        Self::Blocked,
        Self::Review,
        Self::Merged,
        Self::Released,
        Self::Cancelled,
    ];

    /// The board column for this status.
    pub fn category(self) -> StatusCategory {
        match self {
            Self::Backlog | Self::Ready => StatusCategory::ToDo,
            Self::Running | Self::Waiting | Self::Blocked => StatusCategory::InProgress,
            Self::Review => StatusCategory::InReview,
            Self::Merged | Self::Released | Self::Cancelled => StatusCategory::Done,
        }
    }

    /// Statuses where the chain is waiting on a person (shown as "needs a human").
    pub fn needs_human(self) -> bool {
        matches!(self, Self::Waiting | Self::Blocked | Self::Review)
    }
}

/// Why a ticket is Waiting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WaitingReason {
    /// The agent asked a question (`ask_human`).
    Question { text: String },
    /// The agent wants permission for an action.
    Permission { text: String },
    /// The agent stopped responding to nudges.
    Silent,
}

/// Why a ticket is Blocked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BlockedReason {
    /// A person blocked it by hand.
    Manual { text: String },
    /// The runner process crashed.
    Crashed { detail: String },
    /// A dependency was reopened after it had merged.
    DependencyReopened { dependency: TicketKey },
    /// Merging into the work branch conflicted.
    MergeConflict { detail: String },
    /// The daemon restarted and the runner was gone.
    RunnerLost,
}

/// The reason attached to the current status, if that status carries one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum StatusReason {
    Waiting(WaitingReason),
    Blocked {
        reason: BlockedReason,
        /// Status to return to on unblock.
        resume_to: TicketStatus,
        /// The open question when it was blocked from Waiting, restored on unblock.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        waiting: Option<WaitingReason>,
    },
    Cancelled {
        text: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_status_has_the_category_from_the_plan() {
        use StatusCategory::*;
        let expected = [
            ToDo, ToDo, InProgress, InProgress, InProgress, InReview, Done, Done, Done,
        ];
        let actual: Vec<_> = TicketStatus::ALL.iter().map(|s| s.category()).collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn statuses_serialize_snake_case() {
        let json = serde_json::to_string(&TicketStatus::ALL).expect("serialize");
        assert_eq!(
            json,
            r#"["backlog","ready","running","waiting","blocked","review","merged","released","cancelled"]"#
        );
    }
}
