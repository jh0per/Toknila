//! Turning graph steps into domain events: every ticket or milestone a step changed becomes
//! its own event, so intermediate moves (Backlog -> Ready -> Running) are journaled one by one.

use super::*;
use crate::events::DomainEvent;

/// What [`ChainView::settle`] did: each applied step and the events it caused, in order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Settled {
    pub applied: Vec<Applied>,
    pub events: Vec<DomainEvent>,
}

impl ChainView {
    /// Runs `step` and returns its result with an event for every ticket and milestone it
    /// changed, tickets first, each in view order. Created nodes are not reported here; the
    /// caller that adds them journals them.
    pub fn record<R>(
        &mut self,
        step: impl FnOnce(&mut Self) -> Result<R, GraphError>,
    ) -> Result<(R, Vec<DomainEvent>), GraphError> {
        let tickets = self.tickets.clone();
        let releases = self.releases.clone();
        let out = step(self)?;
        let mut events = Vec::new();
        for after in &self.tickets {
            if let Some(before) = tickets.iter().find(|b| b.id == after.id)
                && before != after
            {
                events.push(DomainEvent::ticket_changed(before, after));
            }
        }
        for after in &self.releases {
            if let Some(before) = releases.iter().find(|b| b.id == after.id)
                && before != after
            {
                events.push(DomainEvent::release_changed(before, after));
            }
        }
        Ok((out, events))
    }
}
