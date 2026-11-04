//! Commands: each runs inside one store transaction, goes through the core state machine and
//! chain graph, saves the result and journals every change as an event.

use toknila_core::events::DomainEvent;
use toknila_core::graph::{Action, Applied, ChainView, GraphError};
use toknila_core::machine::{Effect, TicketEvent};
use toknila_core::{
    Actor, AgentProfile, Chain, ChainId, Comment, CommentBody, CommentId, CommentKind, Edge,
    NodeRef, Project, Release, ReleaseId, Subtask, SubtaskId, Ticket, TicketId, TicketKey,
    TicketStatus, Timestamp,
};

use super::error::ApiError;
use crate::store::{Repo, WriteTx};

// ---- references: ids or human names ----

pub fn resolve_project(repo: &Repo<'_>, r: &str) -> Result<Project, ApiError> {
    if let Ok(id) = r.parse()
        && let Some(p) = repo.project(id)?
    {
        return Ok(p);
    }
    repo.project_by_key(&r.to_uppercase())?
        .ok_or_else(|| ApiError::not_found(format!("project {r}")))
}

pub fn resolve_chain(repo: &Repo<'_>, r: &str) -> Result<Chain, ApiError> {
    if let Ok(id) = r.parse()
        && let Some(c) = repo.chain(id)?
    {
        return Ok(c);
    }
    let mut found = repo.chains_named(r)?;
    match found.len() {
        0 => Err(ApiError::not_found(format!("chain {r}"))),
        1 => Ok(found.remove(0)),
        n => Err(ApiError::ambiguous(format!(
            "{n} chains are named {r}; use the chain id"
        ))),
    }
}

pub fn resolve_ticket(repo: &Repo<'_>, r: &str) -> Result<Ticket, ApiError> {
    if let Ok(id) = r.parse()
        && let Some(t) = repo.ticket(id)?
    {
        return Ok(t);
    }
    if let Ok(key) = r.to_uppercase().parse::<TicketKey>()
        && let Some(t) = repo.ticket_by_key(&key)?
    {
        return Ok(t);
    }
    // Last resort: an exact, unique title (handy in short demo chains: `tok sim A finish`).
    let mut found: Vec<Ticket> = repo
        .tickets(None)?
        .into_iter()
        .filter(|t| t.title == r)
        .collect();
    match found.len() {
        0 => Err(ApiError::not_found(format!("ticket {r}"))),
        1 => Ok(found.remove(0)),
        n => Err(ApiError::ambiguous(format!(
            "{n} tickets are titled {r:?}; use the key"
        ))),
    }
}

pub fn resolve_release(repo: &Repo<'_>, r: &str) -> Result<Release, ApiError> {
    if let Ok(id) = r.parse()
        && let Some(rel) = repo.release(id)?
    {
        return Ok(rel);
    }
    let mut found = repo.releases_by_version(r)?;
    match found.len() {
        0 => Err(ApiError::not_found(format!("release {r}"))),
        1 => Ok(found.remove(0)),
        n => Err(ApiError::ambiguous(format!(
            "{n} releases have version {r}; use the release id"
        ))),
    }
}

/// A node of one chain by ticket key or release version.
pub fn resolve_node(view: &ChainView, r: &str) -> Result<NodeRef, ApiError> {
    let upper = r.to_uppercase();
    if let Some(t) = view
        .tickets
        .iter()
        .find(|t| t.key.to_string() == upper || t.id.to_string() == r)
    {
        return Ok(NodeRef::Ticket(t.id));
    }
    if let [t] = view
        .tickets
        .iter()
        .filter(|t| t.title == r)
        .collect::<Vec<_>>()
        .as_slice()
    {
        return Ok(NodeRef::Ticket(t.id));
    }
    if let Some(rel) = view
        .releases
        .iter()
        .find(|x| x.version == r || x.id.to_string() == r)
    {
        return Ok(NodeRef::Release(rel.id));
    }
    Err(ApiError::not_found(format!(
        "{r} in chain {}",
        view.chain.name
    )))
}

// ---- chain transaction ----

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

fn label(view: &ChainView, ticket: TicketId) -> String {
    view.label(NodeRef::Ticket(ticket))
}

/// Loads a chain, runs `f`, lets the graph settle, delivers instructions to simulated agents,
/// saves everything and journals each change.
pub fn with_chain<R>(
    tx: &mut WriteTx<'_>,
    chain: ChainId,
    f: impl FnOnce(&mut ChainTx, &Repo<'_>) -> Result<R, ApiError>,
) -> Result<R, ApiError> {
    let view = tx
        .repo()
        .chain_view(chain)?
        .ok_or_else(|| ApiError::not_found(format!("chain {chain}")))?;
    let mut ctx = ChainTx::new(view, tx.now());
    let out = f(&mut ctx, &tx.repo())?;
    for _ in 0..MAX_ROUNDS {
        ctx.settle()?;
        ctx.view.validate()?;
        tx.repo().save_chain_view(&ctx.view)?;
        emit_changes(tx, &mut ctx)?;
        record_effects(tx, &mut ctx)?;
        if !deliver_to_sim(tx, &mut ctx)? {
            return Ok(out);
        }
    }
    Err(GraphError::NoFixpoint(MAX_ROUNDS).into())
}

fn emit_changes(tx: &mut WriteTx<'_>, ctx: &mut ChainTx) -> Result<(), ApiError> {
    for event in std::mem::take(&mut ctx.events) {
        tx.emit(&event)?;
    }
    Ok(())
}

fn record_effects(tx: &mut WriteTx<'_>, ctx: &mut ChainTx) -> Result<(), ApiError> {
    for (ticket, effect) in std::mem::take(&mut ctx.effects) {
        let (author, kind, text) = match effect {
            Effect::RecordReport { summary } => (Actor::Agent, CommentKind::Report, summary),
            Effect::RecordInstruction { text } => (Actor::Human, CommentKind::Instruction, text),
            // Phase 0 has no runners; sim needs no process started or stopped.
            _ => continue,
        };
        add_comment(tx, &ctx.view, ticket, author, kind, body_from_text(&text))?;
    }
    Ok(())
}

/// Simulated agents read instructions instantly: pending human comments on a Running or Waiting
/// sim ticket are marked delivered, and a Waiting ticket goes back to Running.
/// Returns whether anything changed.
fn deliver_to_sim(tx: &mut WriteTx<'_>, ctx: &mut ChainTx) -> Result<bool, ApiError> {
    let project = tx.repo().project(ctx.view.chain.project_id)?;
    let default_profile = project
        .map(|p| p.default_profile)
        .unwrap_or_else(|| AgentProfile::SIM.to_owned());
    let candidates: Vec<(TicketId, TicketStatus)> = ctx
        .view
        .tickets
        .iter()
        .filter(|t| matches!(t.status, TicketStatus::Running | TicketStatus::Waiting))
        .filter(|t| t.profile.as_deref().unwrap_or(&default_profile) == AgentProfile::SIM)
        .map(|t| (t.id, t.status))
        .collect();
    let mut changed = false;
    for (ticket, status) in candidates {
        let pending: Vec<Comment> = tx
            .repo()
            .comments(ticket)?
            .into_iter()
            .filter(|c| {
                c.author == Actor::Human
                    && c.kind == CommentKind::Instruction
                    && c.delivered_at.is_none()
            })
            .collect();
        if pending.is_empty() {
            continue;
        }
        for mut c in pending {
            c.delivered_at = Some(tx.now());
            tx.repo().upsert_comment(&c)?;
            let key = label(&ctx.view, ticket);
            tx.emit(&DomainEvent::CommentDelivered {
                ticket: key,
                comment: c,
            })?;
        }
        if status == TicketStatus::Waiting {
            ctx.ticket_event(ticket, TicketEvent::InstructionDelivered)?;
        }
        changed = true;
    }
    Ok(changed)
}

/// First line becomes the gist, the rest the "what" field.
pub fn body_from_text(text: &str) -> CommentBody {
    let text = text.trim();
    let (gist, rest) = text.split_once('\n').unwrap_or((text, ""));
    let rest = rest.trim();
    CommentBody {
        gist: gist.trim().to_owned(),
        what: (!rest.is_empty()).then(|| rest.to_owned()),
        ..CommentBody::default()
    }
}

/// Stores a comment and journals it.
pub fn add_comment(
    tx: &mut WriteTx<'_>,
    view: &ChainView,
    ticket: TicketId,
    author: Actor,
    kind: CommentKind,
    body: CommentBody,
) -> Result<Comment, ApiError> {
    let body = toknila_core::template::normalize(&body);
    if body.gist.is_empty() {
        return Err(ApiError::bad_request(
            "comment needs a gist (first line of text)",
        ));
    }
    let c = Comment {
        id: CommentId::new(),
        ticket_id: ticket,
        author,
        kind,
        body,
        created_at: tx.now(),
        delivered_at: None,
    };
    tx.repo().upsert_comment(&c)?;
    tx.emit(&DomainEvent::CommentAdded {
        ticket: label(view, ticket),
        comment: c.clone(),
    })?;
    Ok(c)
}

/// Inserts a subtask at the end of a ticket's checklist.
pub fn add_subtask(tx: &mut WriteTx<'_>, ticket: &Ticket, text: &str) -> Result<Subtask, ApiError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(ApiError::bad_request("subtask text is empty"));
    }
    let position = u32::try_from(tx.repo().subtasks(ticket.id)?.len())
        .unwrap_or(u32::MAX)
        .saturating_add(1);
    let s = Subtask {
        id: SubtaskId::new(),
        ticket_id: ticket.id,
        position,
        text: text.to_owned(),
        done: false,
        done_by: None,
    };
    tx.repo().upsert_subtask(&s)?;
    tx.emit(&DomainEvent::SubtaskAdded {
        ticket: ticket.key.clone(),
        subtask: s.clone(),
    })?;
    Ok(s)
}

/// Marks a subtask done or not, or renames it.
pub fn update_subtask(
    tx: &mut WriteTx<'_>,
    ticket: &Ticket,
    position: u32,
    done: Option<bool>,
    text: Option<String>,
) -> Result<Subtask, ApiError> {
    let mut s = tx
        .repo()
        .subtasks(ticket.id)?
        .into_iter()
        .find(|s| s.position == position)
        .ok_or_else(|| ApiError::not_found(format!("subtask {position} of {}", ticket.key)))?;
    if let Some(done) = done {
        s.done = done;
        s.done_by = done.then_some(tx.actor());
    }
    if let Some(text) = text {
        s.text = text;
    }
    tx.repo().upsert_subtask(&s)?;
    tx.emit(&DomainEvent::SubtaskUpdated {
        ticket: ticket.key.clone(),
        subtask: s.clone(),
    })?;
    Ok(s)
}
