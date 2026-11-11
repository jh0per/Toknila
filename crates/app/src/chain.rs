//! The chain unit of work: load a chain, change it, let it settle, save it, journal it.

use toknila_core::events::DomainEvent;
use toknila_core::graph::{Action, Applied, ChainView, GraphError};
use toknila_core::machine::{Effect, TicketEvent};
use toknila_core::{
    Actor, Chain, ChainId, CommentKind, Edge, NodeRef, ProjectId, Release, ReleaseId, Ticket,
    TicketId, Timestamp,
};

use crate::notes::{add_comment, body_from_text, deliver_to_sim};
use crate::{
    AppError, AppResult, ErrorKind, UnitOfWork, checks, merges, reviews, runs, schedule, ships,
};

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
    pub(crate) fn new(view: ChainView, now: Timestamp) -> Self {
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

    /// Changes the chain's own settings and journals it.
    pub fn edit_chain(&mut self, f: impl FnOnce(&mut Chain)) {
        f(&mut self.view.chain);
        self.events.push(DomainEvent::ChainUpdated {
            chain: self.view.chain.clone(),
        });
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
    pub fn complete_release(
        &mut self,
        release: ReleaseId,
        tag: Option<String>,
    ) -> Result<(), GraphError> {
        let applied = self.step(|v, now| {
            if let Some(tag) = tag {
                v.tag_release(release, tag)?;
            }
            v.complete_release(release, now)
        })?;
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
/// saves everything and journals each change, then gives free slots of the chain's project to
/// the cards waiting for one ([`crate::schedule`]).
pub fn with_chain<U, R, E>(
    uow: &mut U,
    chain: ChainId,
    f: impl FnOnce(&mut ChainTx) -> Result<R, E>,
) -> Result<R, E>
where
    U: UnitOfWork + ?Sized,
    E: From<AppError>,
{
    let (out, project) = run_chain_in(uow, chain, f)?;
    schedule::run(uow, project)?;
    Ok(out)
}

/// [`with_chain`] without the scheduling step: the scheduler's own way to change a chain.
pub(crate) fn run_chain<U, R, E>(
    uow: &mut U,
    chain: ChainId,
    f: impl FnOnce(&mut ChainTx) -> Result<R, E>,
) -> Result<R, E>
where
    U: UnitOfWork + ?Sized,
    E: From<AppError>,
{
    run_chain_in(uow, chain, f).map(|(out, _)| out)
}

fn run_chain_in<U, R, E>(
    uow: &mut U,
    chain: ChainId,
    f: impl FnOnce(&mut ChainTx) -> Result<R, E>,
) -> Result<(R, ProjectId), E>
where
    U: UnitOfWork + ?Sized,
    E: From<AppError>,
{
    let view = uow
        .chain_view(chain)?
        .ok_or_else(|| AppError::not_found(format!("chain {chain}")))?;
    let project = view.chain.project_id;
    let mut ctx = ChainTx::new(view, uow.now());
    let out = f(&mut ctx)?;
    for _ in 0..MAX_ROUNDS {
        if !round(uow, &mut ctx)? {
            return Ok((out, project));
        }
    }
    Err(AppError::from(GraphError::NoFixpoint(MAX_ROUNDS)).into())
}

/// Feeds an outcome that came back from the host to the ticket through the machine. A ticket
/// that has moved on meanwhile refuses the event; that is not an error of the outcome.
pub(crate) fn tell_ticket<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: TicketId,
    event: TicketEvent,
) -> AppResult<()> {
    let Some(t) = uow.ticket(ticket)? else {
        return Ok(());
    };
    match with_chain(uow, t.chain_id, |tx| {
        tx.ticket_event(ticket, event).map_err(AppError::from)
    }) {
        Err(e) if e.kind == ErrorKind::Conflict => Ok(()),
        other => other,
    }
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
            Effect::PrepareShip => {
                ships::queue_prepare(uow, ctx, ticket, Default::default())?;
                continue;
            }
            Effect::PrepareShipWith { stale } => {
                ships::queue_prepare(uow, ctx, ticket, stale)?;
                continue;
            }
            Effect::Merge => {
                if let Some(t) = ctx.view.tickets.iter().find(|t| t.id == ticket) {
                    merges::queue_merge(uow, &ctx.view.chain, t)?;
                }
                continue;
            }
            Effect::RemoveShip => {
                ships::queue_remove(uow, ctx, ticket)?;
                continue;
            }
            Effect::RetireShip => {
                ships::queue_retire(uow, ctx, ticket)?;
                continue;
            }
            Effect::RecordReport { summary } => (Actor::Agent, CommentKind::Report, summary),
            Effect::RecordInstruction { text } => (Actor::Human, CommentKind::Instruction, text),
            Effect::EnsureRunner => {
                runs::queue_start(uow, ctx, ticket)?;
                continue;
            }
            Effect::StopRunner => {
                runs::queue_stop(uow, ticket);
                continue;
            }
            Effect::StopWorker => {
                runs::queue_stop_worker(uow, ticket);
                continue;
            }
            Effect::StartReviewer => {
                reviews::queue_start(uow, ctx, ticket)?;
                continue;
            }
            Effect::StopReviewer => {
                reviews::queue_stop(uow, ticket);
                continue;
            }
            Effect::NudgeReviewer { message } => {
                reviews::queue_nudge(uow, ticket, message);
                continue;
            }
            Effect::RecordRemarks { text } => (Actor::Agent, CommentKind::Instruction, text),
            Effect::RecordCheckOutput { text } => (Actor::System, CommentKind::Instruction, text),
            Effect::RunCheck => {
                checks::queue_run(uow, ctx, ticket)?;
                continue;
            }
            _ => continue,
        };
        // A hand-in keeps its blocks (what changed, checks, what is left) as the report's fields.
        let body = match kind {
            CommentKind::Report => toknila_core::handin::to_body(&text),
            _ => None,
        };
        let body = body.unwrap_or_else(|| body_from_text(&text));
        add_comment(uow, &ctx.view, ticket, author, kind, body)?;
    }
    Ok(())
}
