//! What the planner reads: the whole plan of its project in one answer.

use serde::{Deserialize, Serialize};
use toknila_core::graph::{ChainSummary, ChainView};
use toknila_core::{
    Chain, NodeRef, Project, ProjectId, Subtask, Thread, ThreadMessage, Ticket, Timestamp,
};

use crate::{AppError, AppResult, Reader};

/// A thread of a card with what was said in it.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanThread {
    pub thread: Thread,
    pub messages: Vec<ThreadMessage>,
    /// How many artifact versions it has; their bodies are read in the window.
    pub versions: u32,
}

/// A card with its place in the plan.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanCard {
    /// The card with its status, prompt, acceptance, review rounds and pull request.
    pub ticket: Ticket,
    /// What it waits for: card keys and `release <version>` labels.
    pub after: Vec<String>,
    pub subtasks: Vec<Subtask>,
    pub threads: Vec<PlanThread>,
}

/// A chain with its cards in dependency order (the order is the priority).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanChain {
    pub chain: Chain,
    pub summary: ChainSummary,
    pub cards: Vec<PlanCard>,
}

/// `GET /v1/planner/plan`: the project and all of its chains.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanView {
    pub project: Project,
    pub chains: Vec<PlanChain>,
}

fn chain_plan<R: Reader + ?Sized>(r: &R, view: ChainView, at: Timestamp) -> AppResult<PlanChain> {
    let summary = view.summary(at);
    let mut cards = Vec::new();
    for node in view.topo_order() {
        let NodeRef::Ticket(id) = node else { continue };
        let Some(ticket) = view.tickets.iter().find(|t| t.id == id).cloned() else {
            continue;
        };
        let after = view
            .edges
            .iter()
            .filter(|e| e.to == node)
            .map(|e| view.label(e.from))
            .collect();
        let mut threads = Vec::new();
        for thread in r.threads(id)? {
            threads.push(PlanThread {
                messages: r.thread_messages(thread.id)?,
                versions: u32::try_from(r.thread_versions(thread.id)?.len()).unwrap_or(u32::MAX),
                thread,
            });
        }
        cards.push(PlanCard {
            subtasks: r.subtasks(id)?,
            ticket,
            after,
            threads,
        });
    }
    Ok(PlanChain {
        chain: view.chain,
        summary,
        cards,
    })
}

/// The plan of `project` at `at`: every chain, every card in order.
pub fn plan<R: Reader + ?Sized>(r: &R, project: ProjectId, at: Timestamp) -> AppResult<PlanView> {
    let project = r
        .project(project)?
        .ok_or_else(|| AppError::not_found("project"))?;
    let mut chains = Vec::new();
    for chain in r.chains(Some(project.id))? {
        if let Some(view) = r.chain_view(chain.id)? {
            chains.push(chain_plan(r, view, at)?);
        }
    }
    Ok(PlanView { project, chains })
}
