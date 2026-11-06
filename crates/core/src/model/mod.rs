//! Domain types. Plain data with serde; rules that change them live in sibling modules.

mod chain;
mod comment;
mod ids;
mod keys;
mod profile;
mod project;
mod release;
mod review;
mod run;
mod ship;
mod status;
mod thread;
mod ticket;
mod time;

pub use chain::{Chain, Edge, EdgeKind, NodeRef};
pub use comment::{Actor, Comment, CommentBody, CommentKind};
pub use ids::{
    ChainId, CommentId, ProjectId, ReleaseId, RunId, SubtaskId, ThreadId, ThreadMessageId,
    ThreadVersionId, TicketId,
};
pub use keys::{KeyError, ProjectKey, TicketKey};
pub use profile::{AgentProfile, AgentRole, HookWiring, McpWiring, Nudge, OutputRules};
pub use project::{Project, ReviewMode};
pub use release::{Release, ReleaseChannel, ReleaseState, StartMode};
pub use review::{
    Review, ReviewRound, ReviewTurn, Verdict, resolve_mode as resolve_review_mode,
    resolve_reviewer, reviewer_profile,
};
pub use run::{ProcessExit, Run};
pub use ship::{Ship, ShipSpec, slugify};
pub use status::{BlockedReason, StatusCategory, StatusReason, TicketStatus, WaitingReason};
pub use thread::{
    ArtifactFormat, MessageKind, Phase, Thread, ThreadKind, ThreadMessage, ThreadState,
    ThreadVersion,
};
pub use ticket::{OutOfOrder, Subtask, Ticket, TicketDraft, TicketPatch, Wildcard};
pub use time::Timestamp;

#[cfg(test)]
mod roundtrip_tests;
