//! Turning a validated plan into a new chain.

use super::{Plan, PlanNode, PlanTicket};
use crate::design::DesignRef;
use crate::graph::ChainView;
use crate::{
    Chain, ChainId, DesignId, Edge, NodeRef, ProjectId, Release, ReleaseId, Ticket, TicketDraft,
    TicketId, TicketKey, Timestamp,
};

/// The subtasks of an imported ticket: text and whether it starts ticked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checklist {
    pub ticket: TicketId,
    pub subtasks: Vec<(String, bool)>,
    /// The file asks for the ticket to be imported as Merged.
    pub done: bool,
    /// Done, but these earlier tickets are not: it is taken to Merged out of order.
    pub behind: Vec<TicketKey>,
    /// Designs the file names that the project does not have: the card was made without them.
    pub dropped_designs: Vec<String>,
}

fn position(index: usize) -> u32 {
    u32::try_from(index + 1).unwrap_or(u32::MAX)
}

impl Plan {
    /// Turns the plan into a new chain of `project`. `key` hands out ticket keys in file order.
    /// Returns the view and the checklist of each ticket.
    pub fn instantiate(
        &self,
        project: ProjectId,
        now: Timestamp,
        key: impl FnMut() -> TicketKey,
    ) -> (ChainView, Vec<Checklist>) {
        self.instantiate_with_designs(project, now, key, &[])
    }

    /// [`Plan::instantiate`] with the project's designs (id, file name): a ticket's design
    /// references are found by name; a name the project lacks is dropped and reported in the
    /// ticket's [`Checklist`].
    pub fn instantiate_with_designs(
        &self,
        project: ProjectId,
        now: Timestamp,
        mut key: impl FnMut() -> TicketKey,
        library: &[(DesignId, String)],
    ) -> (ChainView, Vec<Checklist>) {
        let chain = Chain {
            id: ChainId::new(),
            project_id: project,
            name: self.chain.name.clone(),
            goal: self.chain.goal.clone(),
            autostart: self.chain.autostart,
            review_mode: self.chain.review_mode,
            reviewer: None,
            source: None,
            version: None,
        };
        let tickets: Vec<Ticket> = self
            .tickets
            .iter()
            .map(|t| Ticket::new_backlog(TicketId::new(), chain.id, key(), t.draft(library), now))
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
        let open_before = self.out_of_order();
        let subtasks = tickets
            .iter()
            .zip(&self.tickets)
            .zip(&open_before)
            .map(|((t, p), open)| Checklist {
                ticket: t.id,
                subtasks: p
                    .subtasks
                    .iter()
                    .enumerate()
                    .map(|(i, text)| (text.clone(), p.done_subtasks.contains(&position(i))))
                    .collect(),
                done: p.done,
                behind: open.iter().map(|i| tickets[*i].key.clone()).collect(),
                dropped_designs: p.unknown_designs(library),
            })
            .collect();
        let mut view = ChainView {
            chain,
            tickets,
            releases,
            edges,
            release_tickets: vec![],
        };
        view.unify_end_release(self.chain.version.as_deref());
        (view, subtasks)
    }
}

impl PlanTicket {
    fn unknown_designs(&self, library: &[(DesignId, String)]) -> Vec<String> {
        let known = |name: &str| library.iter().any(|(_, n)| n == name);
        self.designs
            .iter()
            .filter(|d| !known(&d.design))
            .map(|d| d.design.clone())
            .collect()
    }

    fn draft(&self, library: &[(DesignId, String)]) -> TicketDraft {
        let designs = self
            .designs
            .iter()
            .filter_map(|d| {
                let (id, _) = library.iter().find(|(_, n)| *n == d.design)?;
                Some(DesignRef {
                    design: *id,
                    region: d.region,
                })
            })
            .collect();
        TicketDraft {
            designs,
            title: self.title.clone(),
            prompt: self.prompt.clone(),
            acceptance: self.acceptance.clone(),
            paused: self.paused,
            profile: self.profile.clone(),
            review_mode: self.review_mode,
            reviewer: None,
            needs_secrets: self.needs_secrets,
            design_first: self.design_first,
        }
    }
}
