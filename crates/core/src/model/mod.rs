//! Domain types. Plain data with serde; rules that change them live in sibling modules.

mod chain;
mod chain_title;
mod check_outcome;
pub mod command_policy;
mod comment;
mod extra_check;
mod ids;
mod keys;
mod patch;
mod paused;
mod profile;
mod profile_brief;
mod profile_launch;
mod project;
mod release;
mod review;
mod reviewer_rules;
mod reviewer_scripts;
mod run;
mod ship;
mod since;
mod stale;
mod status;
mod thread;
mod ticket;
mod time;

pub use crate::pr::{CheckRun, CheckState, PullRequest};
pub use chain::{Chain, Edge, EdgeKind, NodeRef};
pub use chain_title::{readable_name, slug_from_title};
pub use check_outcome::{CheckOutcome, FLAKY_LINES, Recheck, first_failing_line, last_lines};
pub use command_policy::CommandPolicy;
pub use comment::{Actor, Comment, CommentBody, CommentKind};
pub use extra_check::ExtraCheck;
pub(crate) use ids::next_ulid;
pub use ids::{
    ChainId, CommentId, DesignId, PinId, ProjectId, ReleaseId, RunId, SubtaskId, ThreadId,
    ThreadMessageId, ThreadVersionId, TicketId,
};
pub use keys::{KeyError, ProjectKey, TicketKey};
pub use patch::{SubtaskEdit, TicketPatch};
pub use paused::PausedAgent;
pub use profile::{
    AgentProfile, AgentRole, HookWiring, McpWiring, Nudge, OutputRules, Permissions,
};
pub use project::{MODEL_ALIASES, MergeMethod, MergeMode, MergeSettings, Project, ReviewMode};
pub use release::{Release, ReleaseChannel, ReleaseState, ReleasedWith, StartMode};
pub use review::{
    ACCEPT_WITHDRAWN, CheckPhase, ItemBasis, ItemOutcome, ItemVerdict, ItemsError, Review,
    ReviewNote, ReviewRound, ReviewTurn, Verdict, check_items, effective_verdict, render_items,
    request_is_unfounded, resolve_mode as resolve_review_mode, resolve_reviewer, reviewer_profile,
};
pub use reviewer_rules::{RepoTool, reviewer_check_rules, reviewer_commands};
pub use reviewer_scripts::{hand_in_scripts, script_rule};
pub use run::{ProcessExit, Run};
pub use ship::{MergeSpec, PlannerSite, PlannerSpec, Ship, ShipLook, ShipSpec, slugify};
pub use since::SinceReturn;
pub use stale::{BaseBehind, BaseCourse, StalePolicy, describe as describe_base_behind};
pub use status::{BlockedReason, StatusReason, TicketStatus, WaitingReason};
pub use thread::{
    ArtifactFormat, MessageKind, Phase, Thread, ThreadKind, ThreadMessage, ThreadState,
    ThreadVersion,
};
pub use ticket::{OutOfOrder, Pos, Size, Subtask, Ticket, TicketDraft, Wildcard};
pub use time::Timestamp;

#[cfg(test)]
mod roundtrip_tests;
