//! Domain types. Plain data with serde; rules that change them live in sibling modules.

mod chain;
mod comment;
mod ids;
mod keys;
mod profile;
mod project;
mod release;
mod status;
mod ticket;
mod time;

pub use chain::{Chain, Edge, EdgeKind, NodeRef};
pub use comment::{Actor, Comment, CommentBody, CommentKind};
pub use ids::{ChainId, CommentId, ProjectId, ReleaseId, SubtaskId, TicketId};
pub use keys::{KeyError, ProjectKey, TicketKey};
pub use profile::{AgentProfile, AgentRole, ProfileKind};
pub use project::{Project, ReviewMode};
pub use release::{Release, ReleaseChannel, ReleaseState, StartMode};
pub use status::{BlockedReason, StatusCategory, StatusReason, TicketStatus, WaitingReason};
pub use ticket::{Subtask, Ticket, TicketDraft};
pub use time::Timestamp;

#[cfg(test)]
mod roundtrip_tests;
