use serde::{Deserialize, Serialize};

use super::{ChainId, ProjectId, ReleaseId, ReviewMode, TicketId};

/// A flow of tickets with dependencies and release milestones between them. Its status,
/// progress and "where it stopped" are computed from tickets and releases, never stored.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chain {
    pub id: ChainId,
    pub project_id: ProjectId,
    pub name: String,
    pub goal: String,
    /// Start Ready tickets automatically (on by default).
    pub autostart: bool,
    /// Overrides the project's review mode for every ticket that does not set its own.
    pub review_mode: Option<ReviewMode>,
    /// Overrides the project's reviewer profile for every ticket that does not set its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewer: Option<String>,
    /// The spec folder or chain file the chain was imported from, as an absolute path; `null`
    /// for a chain made in Toknila or imported from pasted text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

impl Chain {
    /// Autostart of a chain that does not say.
    pub const DEFAULT_AUTOSTART: bool = true;
}

/// A node of the chain graph: a ticket or a release milestone.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum NodeRef {
    Ticket(TicketId),
    Release(ReleaseId),
}

/// How strictly `to` waits for `from`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// Wait for the ticket to be Merged, or for the milestone to be cut.
    #[default]
    Normal,
    /// Wait until the milestone is released (for example to test a store build).
    AfterRelease,
}

/// Dependency edge: `to` cannot start before `from` is done.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Edge {
    pub chain_id: ChainId,
    pub from: NodeRef,
    pub to: NodeRef,
    #[serde(default)]
    pub kind: EdgeKind,
}
