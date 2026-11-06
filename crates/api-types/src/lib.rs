//! Request and response bodies of the Toknila daemon API: what crosses the wire between the
//! daemon and its clients (CLI, web page, desktop shell). Command inputs belong to the use cases
//! in `toknila-app` and are re-exported here; this crate adds the read models, errors and journal
//! entries.

use serde::{Deserialize, Serialize};
use toknila_core::graph::{ChainSummary, ChainView};
use toknila_core::profile::{Catalog, ListedProfile, ProfileProblem};
use toknila_core::{
    Actor, Chain, Comment, Edge, EdgeKind, NodeRef, Phase, Release, ReleaseId, Ship, Subtask,
    Thread, ThreadMessage, ThreadVersion, Ticket, TicketId, Timestamp,
};

pub use toknila_app::chains::NewChain;
pub use toknila_app::projects::NewProject;
pub use toknila_app::releases::NewRelease;
pub use toknila_app::threads::{NewMessage, NewThread, NewVersion};
pub use toknila_app::tickets::{NewComment, NewTicket};
pub use toknila_app::wildcards::NewWildcard;
pub use toknila_core::TicketPatch as PatchTicket;
pub use toknila_core::agent::{AgentEvent, Proposal, ReleaseEvent};

/// The profiles folder, the usable profiles and the files that were not.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileList {
    /// Where profile files go.
    pub folder: String,
    pub profiles: Vec<ListedProfile>,
    pub problems: Vec<ProfileProblem>,
}

impl ProfileList {
    /// The catalog of the profiles folder `folder`.
    pub fn new(folder: String, catalog: Catalog) -> Self {
        Self {
            folder,
            profiles: catalog.profiles,
            problems: catalog.problems,
        }
    }
}

/// A chain with its graph and computed status.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseMember {
    pub release: ReleaseId,
    pub ticket: TicketId,
}

/// A chain in a list.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainItem {
    pub chain: Chain,
    pub summary: ChainSummary,
}

/// A ticket with everything attached to it.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketDetail {
    pub ticket: Ticket,
    pub chain: String,
    pub subtasks: Vec<Subtask>,
    pub comments: Vec<Comment>,
    /// Milliseconds in the current status.
    pub in_status_ms: u64,
    /// Computed from the ticket's threads: design while any is open, implement otherwise.
    pub phase: Phase,
    /// The ticket's worktree and branch while it has them; `null` before it started, after it
    /// merged, or when its project has no repository.
    pub ship: Option<Ship>,
}

/// A thread with its messages and artifact versions.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadDetail {
    pub thread: Thread,
    /// Oldest first.
    pub messages: Vec<ThreadMessage>,
    /// By number; immutable.
    pub versions: Vec<ThreadVersion>,
}

/// `GET /v1/tickets/{ticket}/threads`: every thread of a ticket and the ticket's phase.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketThreads {
    /// The ticket's key.
    pub ticket: String,
    pub phase: Phase,
    pub threads: Vec<ThreadDetail>,
}

/// A ticket in a list.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketItem {
    pub ticket: Ticket,
    pub chain: String,
    pub in_status_ms: u64,
    pub needs_human: bool,
}

/// Filters for `GET /v1/tickets`.
#[cfg_attr(feature = "openapi", derive(utoipa::IntoParams))]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TicketQuery {
    /// Chain id or name.
    pub chain: Option<String>,
    /// Only tickets waiting on a person (Proposed, Waiting, Blocked, Review).
    #[serde(default)]
    pub needs_human: bool,
}

/// `POST /v1/chains/{chain}/edges` and the query of `DELETE`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema, utoipa::IntoParams))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EdgeInput {
    /// Ticket key or release version.
    pub from: String,
    /// Ticket key or release version.
    pub to: String,
    #[serde(default)]
    pub kind: EdgeKind,
}

/// Body of ticket actions. `text` is required by block, return and cancel.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ActionInput {
    pub text: Option<String>,
}

/// `POST /v1/tickets/{ticket}/subtasks`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewSubtask {
    pub text: String,
}

/// `PATCH /v1/tickets/{ticket}/subtasks/{position}`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PatchSubtask {
    pub done: Option<bool>,
    pub text: Option<String>,
}

/// Query of `GET /v1/events`.
#[cfg_attr(feature = "openapi", derive(utoipa::IntoParams))]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
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

/// A chain file: body of `POST /v1/projects/{project}/import`, answer of
/// `GET /v1/chains/{chain}/export`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChainFile {
    pub toml: String,
}

impl ChainDetail {
    /// The read model of a chain at `at`: its graph, its summary and its nodes in order.
    pub fn from_view(view: ChainView, at: Timestamp) -> Self {
        let summary = view.summary(at);
        let order = view.topo_order();
        Self {
            release_tickets: view
                .release_tickets
                .iter()
                .copied()
                .map(ReleaseMember::from)
                .collect(),
            chain: view.chain,
            tickets: view.tickets,
            releases: view.releases,
            edges: view.edges,
            summary,
            order,
        }
    }
}

/// Error body of every failed request.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    /// Human-readable message.
    pub error: String,
    /// Stable machine code: not_found, bad_request, conflict, ambiguous, internal, unauthorized.
    pub code: String,
}

/// One journal entry. `seq` is increasing; clients resume from it.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventRecord {
    pub seq: i64,
    pub at: Timestamp,
    pub actor: Actor,
    pub entity: String,
    pub entity_id: String,
    pub kind: String,
    #[cfg_attr(feature = "openapi", schema(value_type = Object))]
    pub payload: serde_json::Value,
}
