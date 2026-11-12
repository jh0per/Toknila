//! What runs in a project: the live runs of its cards and its assistant.

use serde::{Deserialize, Serialize};

use super::{LiveRunner, working_cards};
use crate::graph::ChainView;
use crate::planner::run_key;
use crate::{AgentRole, ProjectId, TicketId, TicketKey, Timestamp};

/// A runner process that is alive, by ticket, role and when it started.
pub type LiveRun = (TicketId, AgentRole, Timestamp);

/// The card a run works for.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunCard {
    pub id: TicketId,
    pub key: TicketKey,
    pub title: String,
}

/// One live run of a project. The assistant (`role: planner`) has no card and holds no slot.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRun {
    /// The card the run works for; none for the assistant.
    pub card: Option<RunCard>,
    /// `worker`, `reviewer` or `planner`.
    pub role: AgentRole,
    /// When the process started.
    pub since: Timestamp,
}

/// The live runs of `project`: its assistant first, then the runs of its working cards in chain
/// order (worker before reviewer).
pub fn project_runs(project: ProjectId, chains: &[ChainView], live: &[LiveRun]) -> Vec<ProjectRun> {
    let mut out: Vec<ProjectRun> = live
        .iter()
        .filter(|(t, role, _)| *t == run_key(project) && *role == AgentRole::Planner)
        .map(|(_, role, since)| ProjectRun {
            card: None,
            role: *role,
            since: *since,
        })
        .collect();
    let pairs: Vec<LiveRunner> = live.iter().map(|(t, r, _)| (*t, *r)).collect();
    for id in working_cards(chains, &pairs) {
        let Some(ticket) = chains.iter().flat_map(|c| &c.tickets).find(|t| t.id == id) else {
            continue;
        };
        for role in [AgentRole::Worker, AgentRole::Reviewer] {
            if let Some((_, _, since)) = live.iter().find(|(t, r, _)| *t == id && *r == role) {
                out.push(ProjectRun {
                    card: Some(RunCard {
                        id,
                        key: ticket.key.clone(),
                        title: ticket.title.clone(),
                    }),
                    role,
                    since: *since,
                });
            }
        }
    }
    out
}
