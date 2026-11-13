//! Chain read models and the inputs that change a chain.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use toknila_core::graph::ChainSummary;
use toknila_core::graph::ChainView;
use toknila_core::usage::{RunTime, UsageFigures};
use toknila_core::{
    Chain, Edge, EdgeKind, NodeRef, Release, ReleaseId, Ticket, TicketId, Timestamp,
};

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
    /// Estimated token usage per card, by ticket key; cards that used none are absent.
    #[serde(default)]
    pub usage: BTreeMap<String, UsageFigures>,
    /// How long each card's agents ran, by ticket key; cards that never ran are absent.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub run_time: BTreeMap<String, RunTime>,
    /// What an import left out (a design the project does not have); only an import answers it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
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
            usage: BTreeMap::new(),
            run_time: BTreeMap::new(),
            warnings: Vec::new(),
        }
    }
}

/// One ticket that went into a release.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseMember {
    pub release: ReleaseId,
    pub ticket: TicketId,
}

impl From<(ReleaseId, TicketId)> for ReleaseMember {
    fn from((release, ticket): (ReleaseId, TicketId)) -> Self {
        Self { release, ticket }
    }
}

/// A chain in a list.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainItem {
    pub chain: Chain,
    pub summary: ChainSummary,
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

/// `POST /v1/chains/{chain}/edges/move`: replace a dependency by another, all or nothing.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EdgeMoveInput {
    /// The dependency that is dropped (its `kind` is ignored).
    pub old: EdgeInput,
    /// The dependency that takes its place.
    pub new: EdgeInput,
}

/// `POST /v1/tickets/{ticket}/designs/{index}/move`: give a design reference to another card.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DesignMoveInput {
    /// Key of the card of the same chain that receives the reference.
    pub target: String,
}

/// `POST /v1/chains/{chain}/tickets/{ticket}/move`: put the card directly before or after
/// another card of the chain.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MoveInput {
    /// Ticket key of the card the moved card goes next to.
    pub anchor: String,
    pub place: toknila_core::graph::Place,
}

/// `POST /v1/tickets/{ticket}/move-to-chain`: a card that has not started goes to another epic
/// of its project: to the end, or next to a card of that epic.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MoveToChain {
    /// The epic (id or name) that receives the card.
    pub chain: String,
    /// Where it goes in that epic; left out, at the end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<MoveInput>,
}

/// `POST /v1/planner/chains/{chain}/cards/{card}/move`: the planner puts a card next to another
/// of its epic, or moves it to another epic (`to_chain`: at the end unless `anchor` and `place`
/// say where).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct PlannerMove {
    /// Ticket key of the card the moved card goes next to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<toknila_core::graph::Place>,
    /// The epic the card moves to, when it leaves its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_chain: Option<String>,
}
