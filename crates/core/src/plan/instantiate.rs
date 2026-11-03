//! Turning a validated plan into a new chain.

use super::{Plan, PlanNode, PlanTicket};
use crate::graph::ChainView;
use crate::{
    Chain, ChainId, Edge, NodeRef, ProjectId, Release, ReleaseId, Ticket, TicketDraft, TicketId,
    TicketKey, Timestamp,
};

impl Plan {
    /// Turns the plan into a new chain of `project`. `key` hands out ticket keys in file order.
    /// Returns the view and the subtask texts of each ticket.
    pub fn instantiate(
        &self,
        project: ProjectId,
        now: Timestamp,
        mut key: impl FnMut() -> TicketKey,
    ) -> (ChainView, Vec<(TicketId, Vec<String>)>) {
        let chain = Chain {
            id: ChainId::new(),
            project_id: project,
            name: self.chain.name.clone(),
            goal: self.chain.goal.clone(),
            autostart: self.chain.autostart,
            review_mode: self.chain.review_mode,
        };
        let tickets: Vec<Ticket> = self
            .tickets
            .iter()
            .map(|t| Ticket::new_backlog(TicketId::new(), chain.id, key(), t.draft(), now))
            .collect();
        let releases: Vec<Release> = self
            .releases
            .iter()
            .map(|r| {
                let id = ReleaseId::new();
                Release::planned(id, chain.id, r.version.clone(), r.channel, r.start_mode)
            })
            .collect();
        let node = |n: PlanNode| match n {
            PlanNode::Ticket(i) => NodeRef::Ticket(tickets[i].id),
            PlanNode::Release(i) => NodeRef::Release(releases[i].id),
        };
        let edges = self
            .edges
            .iter()
            .map(|e| Edge {
                chain_id: chain.id,
                from: node(e.from),
                to: node(e.to),
                kind: e.kind,
            })
            .collect();
        let subtasks = tickets
            .iter()
            .zip(&self.tickets)
            .map(|(t, p)| (t.id, p.subtasks.clone()))
            .collect();
        let view = ChainView {
            chain,
            tickets,
            releases,
            edges,
            release_tickets: vec![],
        };
        (view, subtasks)
    }
}

impl PlanTicket {
    fn draft(&self) -> TicketDraft {
        TicketDraft {
            title: self.title.clone(),
            prompt: self.prompt.clone(),
            acceptance: self.acceptance.clone(),
            paused: self.paused,
            profile: self.profile.clone(),
            review_mode: self.review_mode,
            needs_secrets: self.needs_secrets,
        }
    }
}
