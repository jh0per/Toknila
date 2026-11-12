use serde::{Deserialize, Serialize};

use super::{ChainId, ProjectId, ReleaseId, ReviewMode, TicketId};

/// A flow of tickets with dependencies and release milestones between them. Its status,
/// progress and "where it stopped" are computed from tickets and releases, never stored.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chain {
    pub id: ChainId,
    pub project_id: ProjectId,
    /// The identifier of folders and branches (`ship/<name>/...`); never shown as the epic's
    /// name where a person reads it, see [`Chain::display_title`].
    pub name: String,
    /// The words a person reads; none in a row older than titles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
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
    /// The version the chain releases, such as `0.3.0`: the epic's release. Its end release
    /// carries the same version; none for a chain that was made without one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Where the chain stands in its project's roadmap, from 0; chains are listed by it. A new
    /// or imported chain takes the place after the last one.
    #[serde(default)]
    pub position: u32,
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
