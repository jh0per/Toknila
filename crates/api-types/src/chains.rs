//! Chain read models and the inputs that change a chain.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use toknila_core::graph::ChainSummary;
use toknila_core::graph::ChainView;
use toknila_core::usage::UsageFigures;
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

/// `POST /v1/chains/{chain}/tickets/{ticket}/move`: put the card directly before or after
/// another card of the chain.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MoveInput {
    /// Ticket key of the card the moved card goes next to.
    pub anchor: String,
    pub place: toknila_core::graph::Place,
}
