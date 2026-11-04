use std::fmt;

use serde::{Deserialize, Serialize};

use super::TicketKey;

/// Ticket status. Transitions between them are owned by the state machine (P0-04).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TicketStatus {
    /// A wildcard card waiting for a person to approve it; not part of the plan yet.
    Proposed,
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
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
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
    pub const ALL: [TicketStatus; 10] = [
        Self::Proposed,
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
            Self::Proposed | Self::Backlog | Self::Ready => StatusCategory::ToDo,
            Self::Running | Self::Waiting | Self::Blocked => StatusCategory::InProgress,
            Self::Review => StatusCategory::InReview,
            Self::Merged | Self::Released | Self::Cancelled => StatusCategory::Done,
        }
    }

    /// Done for its dependents: in the work branch (Merged, Released) or dropped (Cancelled).
    pub fn is_done(self) -> bool {
        matches!(self, Self::Merged | Self::Released | Self::Cancelled)
    }

    /// Statuses where the chain is waiting on a person (shown as "needs a human").
    pub fn needs_human(self) -> bool {
        matches!(
            self,
            Self::Proposed | Self::Waiting | Self::Blocked | Self::Review
        )
    }
}

/// Why a ticket is Waiting.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
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
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
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
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
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
    /// A wildcard card waits for a person to approve or reject it.
    Proposed,
}

impl fmt::Display for WaitingReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Question { text } => write!(f, "question: {text}"),
            Self::Permission { text } => write!(f, "permission: {text}"),
            Self::Silent => f.write_str("silent"),
        }
    }
}

impl fmt::Display for BlockedReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Manual { text } => write!(f, "manual: {text}"),
            Self::Crashed { detail } => write!(f, "crashed: {detail}"),
            Self::DependencyReopened { dependency } => {
                write!(f, "dependency {dependency} reopened")
            }
            Self::MergeConflict { detail } => write!(f, "merge_conflict: {detail}"),
            Self::RunnerLost => f.write_str("runner lost"),
        }
    }
}

/// The reason in a few words, as the CLI shows it (`question: which framework?`).
impl fmt::Display for StatusReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Waiting(w) => w.fmt(f),
            Self::Blocked { reason, .. } => reason.fmt(f),
            Self::Cancelled { text } => write!(f, "cancelled: {text}"),
            Self::Proposed => f.write_str("awaiting approval"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_status_has_the_category_from_the_plan() {
        use StatusCategory::*;
        let expected = [
            ToDo, ToDo, ToDo, InProgress, InProgress, InProgress, InReview, Done, Done, Done,
        ];
        let actual: Vec<_> = TicketStatus::ALL.iter().map(|s| s.category()).collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn done_means_merged_released_or_cancelled() {
        let done: Vec<_> = TicketStatus::ALL
            .into_iter()
            .filter(|s| s.is_done())
            .collect();
        assert_eq!(
            done,
            [
                TicketStatus::Merged,
                TicketStatus::Released,
                TicketStatus::Cancelled
            ]
        );
    }

    #[test]
    fn reasons_read_in_a_few_words() {
        let blocked = |reason| StatusReason::Blocked {
            reason,
            resume_to: TicketStatus::Running,
            waiting: None,
        };
        let dep: TicketKey = "DEMO-2".parse().expect("key");
        let cases = [
            (
                StatusReason::Waiting(WaitingReason::Question {
                    text: "which?".into(),
                }),
                "question: which?",
            ),
            (
                StatusReason::Waiting(WaitingReason::Permission { text: "rm".into() }),
                "permission: rm",
            ),
            (StatusReason::Waiting(WaitingReason::Silent), "silent"),
            (
                blocked(BlockedReason::Manual {
                    text: "wait".into(),
                }),
                "manual: wait",
            ),
            (
                blocked(BlockedReason::Crashed {
                    detail: "exit 1".into(),
                }),
                "crashed: exit 1",
            ),
            (
                blocked(BlockedReason::DependencyReopened { dependency: dep }),
                "dependency DEMO-2 reopened",
            ),
            (
                blocked(BlockedReason::MergeConflict {
                    detail: "a.rs".into(),
                }),
                "merge_conflict: a.rs",
            ),
            (blocked(BlockedReason::RunnerLost), "runner lost"),
            (
                StatusReason::Cancelled { text: "dup".into() },
                "cancelled: dup",
            ),
            (StatusReason::Proposed, "awaiting approval"),
        ];
        for (reason, text) in cases {
            assert_eq!(reason.to_string(), text);
        }
    }

    #[test]
    fn statuses_serialize_snake_case() {
        let json = serde_json::to_string(&TicketStatus::ALL).expect("serialize");
        assert_eq!(
            json,
            r#"["proposed","backlog","ready","running","waiting","blocked","review","merged","released","cancelled"]"#
        );
        assert_eq!(
            serde_json::to_string(&StatusReason::Proposed).expect("serialize"),
            r#"{"status":"proposed"}"#
        );
    }

    #[test]
    fn a_proposed_card_needs_a_human() {
        let human: Vec<_> = TicketStatus::ALL
            .into_iter()
            .filter(|s| s.needs_human())
            .collect();
        assert_eq!(
            human,
            [
                TicketStatus::Proposed,
                TicketStatus::Waiting,
                TicketStatus::Blocked,
                TicketStatus::Review
            ]
        );
    }
}
