//! Request and response bodies of the API.

use serde::{Deserialize, Serialize};
use toknila_core::graph::ChainSummary;
use toknila_core::{
    Chain, Comment, Edge, EdgeKind, NodeRef, Project, Release, ReleaseId, Subtask, Ticket, TicketId,
};
use utoipa::{IntoParams, ToSchema};

pub use toknila_app::chains::NewChain;
pub use toknila_app::projects::NewProject;
pub use toknila_app::releases::NewRelease;
pub use toknila_app::tickets::{NewComment, NewTicket};
pub use toknila_core::TicketPatch as PatchTicket;

/// A chain with its graph and computed status.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChainDetail {
    pub chain: Chain,
    pub tickets: Vec<Ticket>,
    pub releases: Vec<Release>,
    pub edges: Vec<Edge>,
    pub release_tickets: Vec<ReleaseMember>,
    pub summary: ChainSummary,
    /// Nodes in dependency order.
    pub order: Vec<NodeRef>,
}

/// One ticket that went into a release.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReleaseMember {
    pub release: ReleaseId,
    pub ticket: TicketId,
}

/// A chain in a list.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChainItem {
    pub chain: Chain,
    pub summary: ChainSummary,
}

/// A ticket with everything attached to it.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TicketDetail {
    pub ticket: Ticket,
    pub chain: String,
    pub subtasks: Vec<Subtask>,
    pub comments: Vec<Comment>,
    /// Milliseconds in the current status.
    pub in_status_ms: u64,
}

/// A ticket in a list.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TicketItem {
    pub ticket: Ticket,
    pub chain: String,
    pub in_status_ms: u64,
    pub needs_human: bool,
}

/// Filters for `GET /v1/tickets`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, IntoParams)]
pub struct TicketQuery {
    /// Chain id or name.
    pub chain: Option<String>,
    /// Only tickets waiting on a person (Waiting, Blocked, Review).
    #[serde(default)]
    pub needs_human: bool,
}

/// `POST /v1/chains/{chain}/edges` and the query of `DELETE`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema, IntoParams)]
pub struct EdgeInput {
    /// Ticket key or release version.
    pub from: String,
    /// Ticket key or release version.
    pub to: String,
    #[serde(default)]
    pub kind: EdgeKind,
}

/// Body of ticket actions. `text` is required by block, return and cancel.
#[derive(Debug, Clone, Default, Deserialize, Serialize, ToSchema)]
pub struct ActionInput {
    pub text: Option<String>,
}

/// `POST /v1/tickets/{ticket}/subtasks`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct NewSubtask {
    pub text: String,
}

/// `PATCH /v1/tickets/{ticket}/subtasks/{position}`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct PatchSubtask {
    pub done: Option<bool>,
    pub text: Option<String>,
}

/// Query of `GET /v1/events`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, IntoParams)]
pub struct EventQuery {
    /// Return events with seq greater than this.
    #[serde(default)]
    pub since: i64,
    /// At most this many (default 1000).
    pub limit: Option<usize>,
}

impl From<(ReleaseId, TicketId)> for ReleaseMember {
    fn from((release, ticket): (ReleaseId, TicketId)) -> Self {
        Self { release, ticket }
    }
}

/// Project returned on create.
pub type ProjectOut = Project;

/// A chain file: body of `POST /v1/projects/{project}/import`, answer of
/// `GET /v1/chains/{chain}/export`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ChainFile {
    pub toml: String,
}
