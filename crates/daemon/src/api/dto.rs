//! Request and response bodies of the API.

use serde::{Deserialize, Serialize};
use toknila_core::graph::ChainSummary;
use toknila_core::{
    Chain, Comment, CommentBody, CommentKind, Edge, EdgeKind, NodeRef, Project, Release,
    ReleaseChannel, ReleaseId, ReviewMode, StartMode, Subtask, Ticket, TicketId,
};
use utoipa::{IntoParams, ToSchema};

/// `POST /v1/projects`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct NewProject {
    /// Short uppercase key, prefix of ticket keys (`DEMO`).
    pub key: String,
    pub name: Option<String>,
    pub repo_path: Option<String>,
    pub work_branch: Option<String>,
    pub release_branch: Option<String>,
    pub default_profile: Option<String>,
    pub default_review_mode: Option<ReviewMode>,
}

/// `POST /v1/projects/{project}/chains`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct NewChain {
    pub name: String,
    pub goal: Option<String>,
    /// Defaults to true.
    pub autostart: Option<bool>,
    pub review_mode: Option<ReviewMode>,
}

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

/// `POST /v1/chains/{chain}/tickets`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, ToSchema)]
pub struct NewTicket {
    pub title: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub acceptance: Vec<String>,
    /// Dependencies: ticket keys or release versions of the same chain.
    #[serde(default)]
    pub after: Vec<String>,
    /// Checklist items.
    #[serde(default)]
    pub subtasks: Vec<String>,
    pub profile: Option<String>,
    pub review_mode: Option<ReviewMode>,
    #[serde(default)]
    pub needs_secrets: bool,
    #[serde(default)]
    pub paused: bool,
}

/// `PATCH /v1/tickets/{ticket}`: only the given fields change.
#[derive(Debug, Clone, Default, Deserialize, Serialize, ToSchema)]
pub struct PatchTicket {
    pub title: Option<String>,
    pub prompt: Option<String>,
    pub acceptance: Option<Vec<String>>,
    pub profile: Option<String>,
    pub review_mode: Option<ReviewMode>,
    pub needs_secrets: Option<bool>,
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

/// `POST /v1/chains/{chain}/releases`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct NewRelease {
    pub version: String,
    /// Defaults to tag_only.
    pub channel: Option<ReleaseChannel>,
    /// Defaults to manual.
    pub start_mode: Option<StartMode>,
    /// Nodes the milestone waits for (ticket keys or versions).
    #[serde(default)]
    pub after: Vec<String>,
    /// Nodes that wait for the milestone.
    #[serde(default)]
    pub before: Vec<String>,
}

/// Body of ticket actions. `text` is required by block, return and cancel.
#[derive(Debug, Clone, Default, Deserialize, Serialize, ToSchema)]
pub struct ActionInput {
    pub text: Option<String>,
}

/// `POST /v1/tickets/{ticket}/comments`. Give `text` (first line is the gist, the rest becomes
/// "what") or a full `body`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, ToSchema)]
pub struct NewComment {
    /// Defaults to instruction.
    pub kind: Option<CommentKind>,
    pub text: Option<String>,
    pub body: Option<CommentBody>,
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
