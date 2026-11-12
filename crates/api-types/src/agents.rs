//! The agent slots of a project and the cards waiting for one.

use serde::{Deserialize, Serialize};
use toknila_core::schedule::{Plan, ProjectRun};
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
    /// What runs in the project now: the assistant first (`role: planner`, no card), then the
    /// runs of its cards. The assistant holds no slot: it is in `runs`, never in `used`.
    pub runs: Vec<ProjectRun>,
}

impl ProjectAgents {
    /// The slots of `project` as `plan` computed them, and the `runs` that are live in it.
    pub fn new(project: &Project, plan: &Plan, runs: Vec<ProjectRun>) -> Self {
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
            runs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use toknila_core::schedule::RunCard;
    use toknila_core::{AgentRole, Timestamp};

    #[test]
    fn the_runs_of_a_project_are_pinned_on_the_wire() {
        let project = ProjectId::new();
        let card = TicketId::new();
        let runs = vec![
            ProjectRun {
                card: None,
                role: AgentRole::Planner,
                since: Timestamp(5),
            },
            ProjectRun {
                card: Some(RunCard {
                    id: card,
                    key: "DEMO-1".parse().expect("key"),
                    title: "Add login".to_owned(),
                }),
                role: AgentRole::Reviewer,
                since: Timestamp(7),
            },
        ];
        let agents = ProjectAgents {
            project,
            limit: 2,
            used: 1,
            queued: Vec::new(),
            runs,
        };
        let wire = serde_json::to_string(&agents).expect("json");
        let expected = format!(
            r#"{{"project":"{project}","limit":2,"used":1,"queued":[],"runs":[{{"card":null,"role":"planner","since":5}},{{"card":{{"id":"{card}","key":"DEMO-1","title":"Add login"}},"role":"reviewer","since":7}}]}}"#
        );
        assert_eq!(wire, expected);
    }
}
