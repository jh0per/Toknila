//! The chain unit of work: load a chain, change it, let it settle, save it, journal it.

use toknila_core::events::DomainEvent;
use toknila_core::graph::{Action, Applied, ChainView, GraphError};
use toknila_core::machine::{Effect, TicketEvent};
use toknila_core::{
    Actor, ChainId, CommentKind, Edge, NodeRef, Release, ReleaseId, Ticket, TicketId, Timestamp,
};

use crate::notes::{add_comment, body_from_text, deliver_to_sim};
use crate::{AppError, UnitOfWork};

/// A chain being changed by one command. Every step is journaled by the graph
/// ([`ChainView::record`]) so each transition, including intermediate ones
/// (Backlog -> Ready -> Running), becomes its own event.
#[derive(Debug)]
pub struct ChainTx {
    pub view: ChainView,
    now: Timestamp,
    events: Vec<DomainEvent>,
    effects: Vec<(TicketId, Effect)>,
}

/// Settle-and-deliver rounds one command may take before it is declared stuck.
const MAX_ROUNDS: usize = 10_000;

impl ChainTx {
    fn new(view: ChainView, now: Timestamp) -> Self {
        Self {
            view,
            now,
            events: Vec::new(),
            effects: Vec::new(),
        }
    }

    fn step<R>(
        &mut self,
        f: impl FnOnce(&mut ChainView, Timestamp) -> Result<R, GraphError>,
    ) -> Result<R, GraphError> {
        let now = self.now;
        let (out, events) = self.view.record(|v| f(v, now))?;
        self.events.extend(events);
        Ok(out)
    }

    fn keep_effects(&mut self, applied: Vec<Applied>) {
        for a in applied {
            if let Action::Ticket { ticket, .. } = a.action {
                self.effects
                    .extend(a.effects.into_iter().map(|e| (ticket, e)));
            }
        }
    }

    /// Applies an event to a ticket through the state machine.
    pub fn ticket_event(&mut self, ticket: TicketId, event: TicketEvent) -> Result<(), GraphError> {
        let effects = self.step(|v, now| v.apply_ticket_event(ticket, &event, now))?;
        self.effects
            .extend(effects.into_iter().map(|e| (ticket, e)));
        Ok(())
    }

    /// Changes a ticket's plain fields (not its status).
    pub fn edit_ticket(
        &mut self,
        ticket: TicketId,
        f: impl FnOnce(&mut Ticket),
    ) -> Result<(), GraphError> {
        self.step(|v, _| {
            let t = v
                .tickets
                .iter_mut()
                .find(|t| t.id == ticket)
                .ok_or(GraphError::UnknownTicket(ticket))?;
            f(t);
            Ok(())
        })
    }

    /// Cuts a milestone (`release start`).
    pub fn cut_release(&mut self, release: ReleaseId) -> Result<(), GraphError> {
        self.step(|v, now| v.cut_release(release, now))
    }

    /// Marks a cut milestone released; its tickets move to Released.
    pub fn complete_release(&mut self, release: ReleaseId) -> Result<(), GraphError> {
        let applied = self.step(|v, now| v.complete_release(release, now))?;
        self.keep_effects(applied);
        Ok(())
    }

    /// Adds a new ticket to the chain.
    pub fn add_ticket(&mut self, t: Ticket) {
        self.events
            .push(DomainEvent::TicketCreated { ticket: t.clone() });
        self.view.tickets.push(t);
    }

    /// Adds a wildcard card in Proposed; it stays outside the graph until a person approves it.
    pub fn propose(&mut self, card: Ticket) -> Result<(), GraphError> {
        self.view.propose(card.clone())?;
        self.events
            .push(DomainEvent::TicketProposed { ticket: card });
        Ok(())
    }

    /// Adds a new milestone to the chain.
    pub fn add_release(&mut self, r: Release) {
        self.events
            .push(DomainEvent::ReleaseCreated { release: r.clone() });
        self.view.releases.push(r);
    }

    /// Adds a dependency if the graph stays valid.
    pub fn add_edge(&mut self, e: Edge) -> Result<(), GraphError> {
        self.view.add_edge(e)?;
        self.events.push(DomainEvent::EdgeAdded { edge: e });
        Ok(())
    }

    /// Removes a dependency; `None` if there was none.
    pub fn remove_edge(&mut self, from: NodeRef, to: NodeRef) -> Option<Edge> {
        let idx = self
            .view
            .edges
            .iter()
            .position(|e| e.from == from && e.to == to)?;
        let e = self.view.edges.remove(idx);
        self.events.push(DomainEvent::EdgeRemoved { edge: e });
        Some(e)
    }

    fn settle(&mut self) -> Result<(), GraphError> {
        let settled = self.view.settle(self.now)?;
        self.events.extend(settled.events);
        self.keep_effects(settled.applied);
        Ok(())
    }
}

/// Loads a chain, runs `f`, lets the graph settle, delivers instructions to simulated agents,
/// saves everything and journals each change.
pub fn with_chain<U, R, E>(
    uow: &mut U,
    chain: ChainId,
    f: impl FnOnce(&mut ChainTx) -> Result<R, E>,
) -> Result<R, E>
where
    U: UnitOfWork + ?Sized,
    E: From<AppError>,
{
    let view = uow
        .chain_view(chain)?
        .ok_or_else(|| AppError::not_found(format!("chain {chain}")))?;
    let mut ctx = ChainTx::new(view, uow.now());
    let out = f(&mut ctx)?;
    for _ in 0..MAX_ROUNDS {
        if !round(uow, &mut ctx)? {
            return Ok(out);
        }
    }
    Err(AppError::from(GraphError::NoFixpoint(MAX_ROUNDS)).into())
}

/// One settle-save-journal-deliver round; `true` if delivery changed something.
fn round<U: UnitOfWork + ?Sized>(uow: &mut U, ctx: &mut ChainTx) -> Result<bool, AppError> {
    ctx.settle()?;
    ctx.view.validate()?;
    uow.save_chain_view(&ctx.view)?;
    for event in std::mem::take(&mut ctx.events) {
        uow.emit(&event)?;
    }
    record_effects(uow, ctx)?;
    deliver_to_sim(uow, ctx)
}

fn record_effects<U: UnitOfWork + ?Sized>(uow: &mut U, ctx: &mut ChainTx) -> Result<(), AppError> {
    for (ticket, effect) in std::mem::take(&mut ctx.effects) {
        let (author, kind, text) = match effect {
            Effect::RecordReport { summary } => (Actor::Agent, CommentKind::Report, summary),
            Effect::RecordInstruction { text } => (Actor::Human, CommentKind::Instruction, text),
            // Phase 0 has no runners; sim needs no process started or stopped.
            _ => continue,
        };
        add_comment(uow, &ctx.view, ticket, author, kind, body_from_text(&text))?;
    }
    Ok(())
}
