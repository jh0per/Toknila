//! Turning graph steps into domain events: every ticket, milestone or edge a step changed becomes
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
    /// Runs `step` and returns its result with an event for every ticket, milestone and edge it
    /// changed: tickets first (a decision on a wildcard card before its status change), then
    /// milestones, then added and removed edges, each in view order. Created nodes are not
    /// reported here; the caller that adds them journals them.
    pub fn record<R>(
        &mut self,
        step: impl FnOnce(&mut Self) -> Result<R, GraphError>,
    ) -> Result<(R, Vec<DomainEvent>), GraphError> {
        let tickets = self.tickets.clone();
        let releases = self.releases.clone();
        let edges = self.edges.clone();
        let out = step(self)?;
        let mut events = Vec::new();
        for after in &self.tickets {
            if let Some(before) = tickets.iter().find(|b| b.id == after.id)
                && before != after
            {
                events.extend(DomainEvent::wildcard_decided(before, after));
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
        let added = self.edges.iter().filter(|e| !edges.contains(e));
        events.extend(added.map(|&edge| DomainEvent::EdgeAdded { edge }));
        let removed = edges.iter().filter(|e| !self.edges.contains(e));
        events.extend(removed.map(|&edge| DomainEvent::EdgeRemoved { edge }));
        Ok((out, events))
    }
}
