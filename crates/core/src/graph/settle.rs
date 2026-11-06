//! Driving the chain forward: next actions, applying them, settling to a fixpoint.

use super::*;

const MAX_SETTLE_STEPS: usize = 10_000;

impl ChainView {
    /// Ready cards that want to run, in dependency order: a person started them
    /// (`Ticket::queued`) or the chain autostarts. Whether a slot is free is the schedule's call
    /// (`crate::schedule::plan`), not the graph's.
    pub fn start_requests(&self) -> Vec<TicketId> {
        let mut out = Vec::new();
        for node in self.topo_order() {
            let NodeRef::Ticket(id) = node else { continue };
            let Some(t) = self.ticket(id) else { continue };
            let wanted = t.status == TicketStatus::Ready
                && !t.paused
                && (t.queued || self.chain.autostart)
                && self.unsatisfied_deps(node).is_empty();
            if wanted {
                out.push(id);
            }
        }
        out
    }

    /// What should happen now, in graph order. Empty when the chain is settled.
    pub fn next_actions(&self) -> Vec<Action> {
        let mut actions = Vec::new();
        for node in self.topo_order() {
            match node {
                NodeRef::Ticket(id) => {
                    let Some(t) = self.ticket(id) else { continue };
                    let deps_ok = self.unsatisfied_deps(node).is_empty();
                    match t.status {
                        TicketStatus::Backlog if deps_ok && !t.paused => {
                            actions.push(Action::Ticket {
                                ticket: id,
                                event: TicketEvent::DepsSatisfied,
                            })
                        }
                        TicketStatus::Ready if !deps_ok => actions.push(Action::Ticket {
                            ticket: id,
                            event: TicketEvent::DepsUnsatisfied,
                        }),
                        _ => {}
                    }
                }
                NodeRef::Release(id) => {
                    let Some(r) = self.release(id) else { continue };
                    if r.state == ReleaseState::Planned
                        && r.start_mode == StartMode::Auto
                        && self.unsatisfied_deps(node).is_empty()
                    {
                        actions.push(Action::CutRelease { release: id });
                    }
                }
            }
        }
        actions
    }

    /// Applies one action. Ticket events go through the state machine.
    pub fn apply_action(
        &mut self,
        action: &Action,
        now: Timestamp,
    ) -> Result<Vec<Effect>, GraphError> {
        match action {
            Action::Ticket { ticket, event } => self.apply_ticket_event(*ticket, event, now),
            Action::CutRelease { release } => {
                self.cut_release(*release, now)?;
                Ok(Vec::new())
            }
        }
    }

    /// Applies an event to one ticket and then lets the graph react to its effects (a reopened
    /// dependency blocks running dependents and sends Ready ones back; an approved wildcard card
    /// gets its proposed edges). A refused reaction leaves the view unchanged.
    pub fn apply_ticket_event(
        &mut self,
        ticket: TicketId,
        event: &TicketEvent,
        now: Timestamp,
    ) -> Result<Vec<Effect>, GraphError> {
        let idx = self
            .tickets
            .iter()
            .position(|t| t.id == ticket)
            .ok_or(GraphError::UnknownTicket(ticket))?;
        let tr = machine::apply(&self.tickets[idx], event, now)?;
        let key = tr.ticket.key.clone();
        let previous = std::mem::replace(&mut self.tickets[idx], tr.ticket);
        for effect in &tr.effects {
            if let Effect::AddProposedEdges { after, before } = effect
                && let Err(e) = self.link_proposed(ticket, after, before)
            {
                self.tickets[idx] = previous;
                return Err(e);
            }
        }
        if tr.effects.contains(&Effect::DependencyReopened) {
            self.reopen_dependents(ticket, &key, now)?;
        }
        Ok(tr.effects)
    }

    fn reopen_dependents(
        &mut self,
        ticket: TicketId,
        key: &TicketKey,
        now: Timestamp,
    ) -> Result<(), GraphError> {
        let dependents: Vec<TicketId> = self
            .edges
            .iter()
            .filter(|e| e.from == NodeRef::Ticket(ticket))
            .filter_map(|e| match e.to {
                NodeRef::Ticket(id) => Some(id),
                NodeRef::Release(_) => None,
            })
            .collect();
        for id in dependents {
            let Some(t) = self.ticket(id) else { continue };
            let event = match t.status {
                TicketStatus::Running | TicketStatus::Waiting => TicketEvent::Block {
                    reason: BlockedReason::DependencyReopened {
                        dependency: key.clone(),
                    },
                },
                TicketStatus::Ready => TicketEvent::DepsUnsatisfied,
                _ => continue,
            };
            self.apply_ticket_event(id, &event, now)?;
        }
        Ok(())
    }

    /// Applies [`Self::next_actions`] until there are none. Returns every applied step and the
    /// events each one caused.
    pub fn settle(&mut self, now: Timestamp) -> Result<Settled, GraphError> {
        let mut out = Settled::default();
        loop {
            let Some(action) = self.next_actions().into_iter().next() else {
                return Ok(out);
            };
            let (effects, events) = self.record(|v| v.apply_action(&action, now))?;
            out.events.extend(events);
            out.applied.push(Applied { action, effects });
            if out.applied.len() >= MAX_SETTLE_STEPS {
                return Err(GraphError::NoFixpoint(out.applied.len()));
            }
        }
    }
}
