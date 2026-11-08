use std::fmt;

use serde::{Deserialize, Serialize};

use super::TicketKey;

/// Ticket status. Transitions between them are owned by the state machine.
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

    /// Done for its dependents: in the work branch (Merged, Released) or dropped (Cancelled).
    pub fn is_done(self) -> bool {
        matches!(self, Self::Merged | Self::Released | Self::Cancelled)
    }

    /// Can a person still change the card's title and description? Not once it is in the work
    /// branch or dropped.
    pub fn can_edit(self) -> bool {
        !self.is_done()
    }

    /// Can a person still reorder the card or change what it waits for? Only before it starts.
    pub fn can_move(self) -> bool {
        matches!(self, Self::Backlog | Self::Ready)
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
    Permission {
        text: String,
        /// The whole command or path when `text` had to be cut short for lists; absent when
        /// `text` already says everything.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
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
    /// The agent's process was ended from outside (terminated or killed by another tool or the
    /// user); `detail` says how. Unblocking starts it again.
    Terminated { detail: String },
    /// A dependency was reopened after it had merged.
    DependencyReopened { dependency: TicketKey },
    /// Merging into the base branch conflicted; `detail` names the files.
    MergeConflict { detail: String },
    /// The base branch is checked out with local changes, so the merge would have to touch the
    /// person's files. Nothing was changed.
    BaseCheckoutDirty { path: String },
    /// The merge could not run or finish (the VCS is missing or too old, a branch is gone, the
    /// base kept moving); `detail` says why.
    MergeFailed { detail: String },
    /// Accept found changes in the ship that are not committed; nothing was pushed or merged.
    /// `files` names them. Unblocking checks again.
    ShipUncommitted { files: Vec<String> },
    /// Accept found no commit on the ship's branch: there is nothing to open a pull request for
    /// or to merge. Unblocking checks again.
    ShipEmpty,
    /// The pull request's checks failed (pull-request mode); `checks` names the failed ones.
    /// Unblocking pushes the ship again and watches the pull request again.
    CiFailed { url: String, checks: Vec<String> },
    /// The pull request was closed on GitHub without merging.
    PrClosed { url: String },
    /// The daemon restarted and the runner was gone.
    RunnerLost,
    /// The ship (worktree and branch) could not be prepared.
    ShipFailed { detail: String },
    /// The base branch is behind its upstream (local merge mode), so the ship would start on old
    /// code. Nothing was made. A person fast-forwards the base and starts, or starts anyway;
    /// plain unblock looks again.
    BaseBehind {
        base: String,
        upstream: String,
        behind: u32,
        /// The base can be moved to the upstream without losing anything.
        can_fast_forward: bool,
    },
    /// The runner has no directory to start in: the project has no repository, so the ticket has
    /// no ship. Unblocking starts the ship and the runner again.
    NoWorkingDir,
}

impl BlockedReason {
    /// True for the reasons a merge ends with: a card in Review can be blocked by these only, and
    /// unblocking one tries the merge again.
    pub fn is_merge(&self) -> bool {
        matches!(
            self,
            Self::MergeConflict { .. }
                | Self::BaseCheckoutDirty { .. }
                | Self::MergeFailed { .. }
                | Self::ShipUncommitted { .. }
                | Self::ShipEmpty
                | Self::CiFailed { .. }
                | Self::PrClosed { .. }
        )
    }
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
            Self::Permission { text, .. } => write!(f, "permission: {text}"),
            Self::Silent => f.write_str("silent"),
        }
    }
}

impl fmt::Display for BlockedReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Manual { text } => write!(f, "manual: {text}"),
            Self::Crashed { detail } => write!(f, "crashed: {detail}"),
            Self::Terminated { detail } => write!(
                f,
                "terminated: the agent was terminated from outside ({detail}); unblock starts it again"
            ),
            Self::DependencyReopened { dependency } => {
                write!(f, "dependency {dependency} reopened")
            }
            Self::MergeConflict { detail } => write!(f, "merge_conflict: {detail}"),
            Self::BaseCheckoutDirty { path } => {
                write!(
                    f,
                    "base_checkout_dirty: {path} has local changes; commit or stash them, then unblock"
                )
            }
            Self::MergeFailed { detail } => write!(f, "merge_failed: {detail}"),
            Self::ShipUncommitted { files } => write!(
                f,
                "ship_uncommitted: the ship has changes that are not committed ({}); commit them on the ship, then unblock",
                crate::guard::list_files(files)
            ),
            Self::ShipEmpty => f.write_str(
                "ship_empty: the ship's branch has no commit of its own, so there is nothing to merge; the work has to be committed on the ship, then unblock",
            ),
            Self::CiFailed { url, checks } => write!(
                f,
                "ci_failed: {} failed on {url}; fix it on the ship, then unblock",
                checks.join(", ")
            ),
            Self::PrClosed { url } => write!(
                f,
                "pr_closed: {url} was closed without merging; reopen it or unblock to open a new one"
            ),
            Self::RunnerLost => f.write_str("runner lost"),
            Self::ShipFailed { detail } => write!(f, "ship_failed: {detail}"),
            Self::BaseBehind {
                base,
                upstream,
                behind,
                can_fast_forward,
            } => write!(
                f,
                "base_behind: {}; {}start anyway, or unblock to look again",
                crate::model::describe_base_behind(base, upstream, *behind),
                if *can_fast_forward {
                    "fast-forward it and start, "
                } else {
                    ""
                }
            ),
            Self::NoWorkingDir => f.write_str(
                "no_working_dir: the project has no repository, so the ticket has no ship; give it one or use the sim profile",
            ),
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
                StatusReason::Waiting(WaitingReason::Permission {
                    text: "rm".into(),
                    detail: None,
                }),
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
                blocked(BlockedReason::Terminated {
                    detail: "exit 143, SIGTERM".into(),
                }),
                "terminated: the agent was terminated from outside (exit 143, SIGTERM); unblock starts it again",
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
            (
                blocked(BlockedReason::BaseCheckoutDirty {
                    path: "/repo".into(),
                }),
                "base_checkout_dirty: /repo has local changes; commit or stash them, then unblock",
            ),
            (
                blocked(BlockedReason::MergeFailed {
                    detail: "too old".into(),
                }),
                "merge_failed: too old",
            ),
            (
                blocked(BlockedReason::ShipUncommitted {
                    files: vec!["a.rs".into(), "b.rs".into()],
                }),
                "ship_uncommitted: the ship has changes that are not committed (a.rs, b.rs); commit them on the ship, then unblock",
            ),
            (
                blocked(BlockedReason::ShipEmpty),
                "ship_empty: the ship's branch has no commit of its own, so there is nothing to merge; the work has to be committed on the ship, then unblock",
            ),
            (
                blocked(BlockedReason::BaseBehind {
                    base: "main".into(),
                    upstream: "origin/main".into(),
                    behind: 133,
                    can_fast_forward: true,
                }),
                "base_behind: base branch main is 133 commits behind origin/main; fast-forward it and start, start anyway, or unblock to look again",
            ),
            (blocked(BlockedReason::RunnerLost), "runner lost"),
            (
                blocked(BlockedReason::ShipFailed {
                    detail: "git not found".into(),
                }),
                "ship_failed: git not found",
            ),
            (
                blocked(BlockedReason::NoWorkingDir),
                "no_working_dir: the project has no repository, so the ticket has no ship; give it one or use the sim profile",
            ),
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
