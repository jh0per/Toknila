//! The agent slots of a project and the cards waiting for one.

use serde::{Deserialize, Serialize};
use toknila_core::schedule::Plan;
use toknila_core::{Project, ProjectId, TicketId};

/// A card waiting for an agent slot.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedCard {
    pub ticket: TicketId,
    /// 1 is next.
    pub position: u32,
}

/// `GET /v1/agents`: one project's agent slots and the cards waiting for one.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAgents {
    pub project: ProjectId,
    /// Agents that may work at once.
    pub limit: u32,
    /// Slots held now (cards Running or Waiting). Can exceed `limit` after the limit was lowered.
    pub used: u32,
    /// Waiting cards, first in line first.
    pub queued: Vec<QueuedCard>,
}

impl ProjectAgents {
    /// The slots of `project` as `plan` computed them.
    pub fn new(project: &Project, plan: &Plan) -> Self {
        let queued = plan
            .queue
            .iter()
            .zip(1..)
            .map(|(ticket, position)| QueuedCard {
                ticket: *ticket,
                position,
            });
        Self {
            project: project.id,
            limit: plan.limit,
            used: plan.used,
            queued: queued.collect(),
        }
    }
}
