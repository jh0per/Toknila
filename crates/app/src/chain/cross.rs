//! The two halves of a card changing epic: it leaves one chain and joins another, inside the one
//! command that moves it.

use toknila_core::events::DomainEvent;
use toknila_core::{NodeRef, Ticket, TicketId, TicketKey};

use super::ChainTx;
use crate::{AppError, AppResult};

impl ChainTx {
    /// The keys of the cards of this chain that wait for `ticket` or that it waits for.
    pub fn linked_cards(&self, ticket: TicketId) -> Vec<TicketKey> {
        let me = NodeRef::Ticket(ticket);
        let mut keys: Vec<TicketKey> = self
            .view
            .edges
            .iter()
            .filter(|e| e.from == me || e.to == me)
            .filter_map(|e| match if e.from == me { e.to } else { e.from } {
                NodeRef::Ticket(id) => self.view.tickets.iter().find(|t| t.id == id),
                NodeRef::Release(_) => None,
            })
            .map(|t| t.key.clone())
            .collect();
        keys.sort();
        keys.dedup();
        keys
    }

    /// Takes a card out of the chain. The waits between it and a milestone, and its place in a
    /// cut milestone, go with it; waits between cards must be gone before (see
    /// [`ChainTx::linked_cards`]).
    pub(crate) fn take_ticket(&mut self, ticket: TicketId) -> AppResult<Ticket> {
        let me = NodeRef::Ticket(ticket);
        let ends: Vec<_> = self
            .view
            .edges
            .iter()
            .filter(|e| e.from == me || e.to == me)
            .map(|e| (e.from, e.to))
            .collect();
        for (from, to) in ends {
            self.remove_edge(from, to);
        }
        self.view.release_tickets.retain(|(_, t)| *t != ticket);
        let at = self
            .view
            .tickets
            .iter()
            .position(|t| t.id == ticket)
            .ok_or_else(|| AppError::not_found("ticket"))?;
        Ok(self.view.tickets.remove(at))
    }

    /// Takes a card that came from the epic `from` into the chain, with no waits and no place on
    /// the canvas (the diagram lays it out), and journals the move.
    pub(crate) fn adopt_ticket(&mut self, mut ticket: Ticket, from: &str) {
        ticket.chain_id = self.view.chain.id;
        ticket.pos = None;
        ticket.size = None;
        self.events.push(DomainEvent::TicketMoved {
            from: from.to_owned(),
            to: self.view.chain.name.clone(),
            ticket: ticket.clone(),
        });
        self.view.tickets.push(ticket);
    }
}
