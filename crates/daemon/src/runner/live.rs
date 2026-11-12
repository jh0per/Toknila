//! Which processes are alive, by ticket and role.

use toknila_core::{AgentRole, Run, TicketId};

use super::Runners;

impl Runners {
    /// Tickets with a live worker process.
    pub fn live(&self) -> Vec<TicketId> {
        self.live_in(AgentRole::Worker)
    }

    /// Tickets with a live process of `role`.
    pub fn live_in(&self, role: AgentRole) -> Vec<TicketId> {
        self.active()
            .iter()
            .filter(|((_, r), a)| *r == role && !a.is_over() && !a.stopping())
            .map(|((t, _), _)| *t)
            .collect()
    }

    /// The run of `role` that is alive for `ticket` now, if any.
    pub fn live_run(&self, ticket: TicketId, role: AgentRole) -> Option<Run> {
        let active = self.active().get(&(ticket, role)).cloned();
        active
            .filter(|a| !a.is_over() && !a.stopping())
            .map(|a| a.run().clone())
    }
}
