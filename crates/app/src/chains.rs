//! Chains: create, import and export chain files, edit settings and dependencies.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use toknila_core::events::DomainEvent;
use toknila_core::machine::TicketEvent;
use toknila_core::{
    Chain, ChainId, Edge, EdgeKind, ReviewMode, ReviewTurn, TicketId, TicketKey, TicketStatus,
};

use crate::notes::{add_subtask, update_subtask};
use crate::{AppError, AppResult, ChainTx, Reader, UnitOfWork, resolve, with_chain};

/// `POST /v1/projects/{project}/chains`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewChain {
    pub name: String,
    pub goal: Option<String>,
    /// Defaults to true.
    pub autostart: Option<bool>,
    pub review_mode: Option<ReviewMode>,
    /// Reviewer agent profile for the chain's tickets.
    pub reviewer: Option<String>,
}

/// Creates an empty chain in a project.
pub fn create<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: &str,
    input: NewChain,
) -> AppResult<ChainId> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::bad_request("chain name is empty"));
    }
    let p = resolve::project(uow, project)?;
    let c = Chain {
        id: ChainId::new(),
        project_id: p.id,
        name: name.to_owned(),
        goal: input.goal.unwrap_or_default(),
        autostart: input.autostart.unwrap_or(Chain::DEFAULT_AUTOSTART),
        review_mode: input.review_mode,
        reviewer: input.reviewer.filter(|r| !r.trim().is_empty()),
        source: None,
    };
    uow.insert_chain(&c)?;
    uow.emit(&DomainEvent::ChainCreated { chain: c.clone() })?;
    Ok(c.id)
}

/// Creates a chain from a chain file. Every node and edge goes through the same path as single
/// edits, then the chain settles once (autostart); the whole import lands or nothing does.
/// `source` is the spec folder or chain file the text was read from, kept on the chain.
pub fn import<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: &str,
    toml: &str,
    source: Option<String>,
) -> AppResult<ChainId> {
    let plan = toknila_core::plan::parse(toml).map_err(|e| AppError::bad_request(e.to_string()))?;
    let p = resolve::project(uow, project)?;
    if uow.chain_by_name(p.id, &plan.chain.name)?.is_some() {
        return Err(AppError::conflict(format!(
            "chain {} already exists in {}",
            plan.chain.name, p.key
        )));
    }
    let mut keys = Vec::with_capacity(plan.tickets.len());
    for _ in &plan.tickets {
        let n = uow.take_ticket_number(p.id)?;
        keys.push(
            TicketKey::new(p.key.clone(), n).map_err(|e| AppError::bad_request(e.to_string()))?,
        );
    }
    let mut keys = keys.into_iter();
    let (mut view, subtasks) = plan.instantiate(p.id, uow.now(), || {
        keys.next().expect("one key was taken per ticket")
    });
    view.chain.source = source;
    let (chain_id, chain) = (view.chain.id, view.chain.clone());
    uow.insert_chain(&chain)?;
    uow.emit(&DomainEvent::ChainCreated { chain })?;
    with_chain(uow, chain_id, move |ctx| -> AppResult<()> {
        for t in view.tickets {
            ctx.add_ticket(t);
        }
        for r in view.releases {
            ctx.add_release(r);
        }
        for e in view.edges {
            ctx.add_edge(e)?;
        }
        Ok(())
    })?;
    let mut done = Vec::new();
    let mut late = Vec::new();
    for list in subtasks {
        let ticket = uow
            .ticket(list.ticket)?
            .ok_or_else(|| AppError::not_found("ticket"))?;
        for (text, ticked) in &list.subtasks {
            let s = add_subtask(uow, &ticket, text)?;
            if *ticked {
                update_subtask(uow, &ticket, s.position, Some(true), None)?;
            }
        }
        match (list.done, list.behind.is_empty()) {
            (true, true) => done.push(list.ticket),
            (true, false) => late.push((list.ticket, list.behind)),
            (false, _) => {}
        }
    }
    merge_late(uow, chain_id, late)?;
    merge_done(uow, chain_id, &done)?;
    Ok(chain_id)
}

/// Takes the tickets a file marked done behind open tickets to Merged with the state machine's
/// out-of-order event; the ticket keeps the keys of the open tickets as its mark.
fn merge_late<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    chain: ChainId,
    late: Vec<(TicketId, Vec<TicketKey>)>,
) -> AppResult<()> {
    with_chain(uow, chain, move |ctx| -> AppResult<()> {
        for (id, behind) in late {
            ctx.ticket_event(id, TicketEvent::DoneOutOfOrder { behind })?;
        }
        Ok(())
    })
}

/// Takes the tickets a file marked done through Ready, Running and Review to Merged, with the
/// ordinary events, in dependency order: each round merges what is Ready, and settling then
/// readies its dependents. Done tickets behind open tickets were merged before, out of order.
fn merge_done<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    chain: ChainId,
    done: &[TicketId],
) -> AppResult<()> {
    for _ in 0..=done.len() {
        let moved = with_chain(uow, chain, |ctx| -> AppResult<bool> {
            let mut moved = false;
            for id in done {
                while let Some(event) = next_merge_event(ctx, *id) {
                    ctx.ticket_event(*id, event)?;
                    moved = true;
                }
            }
            Ok(moved)
        })?;
        if !moved {
            break;
        }
    }
    Ok(())
}

fn next_merge_event(ctx: &ChainTx, id: TicketId) -> Option<TicketEvent> {
    let t = ctx.view.tickets.iter().find(|t| t.id == id)?;
    match t.status {
        TicketStatus::Ready => Some(TicketEvent::Start),
        TicketStatus::Running => Some(TicketEvent::Finished {
            summary: "Imported as done: every task was already checked.".to_owned(),
            open_threads: Vec::new(),
            // Nothing was done to review: a person's accept follows at once.
            mode: ReviewMode::Human,
            check: false,
            max_rounds: 0,
        }),
        // The work is already in the repository: the merge is done as soon as it is asked for.
        TicketStatus::Review if t.review.turn == Some(ReviewTurn::Merging) => {
            Some(TicketEvent::MergeDone)
        }
        TicketStatus::Review => Some(TicketEvent::Accept { comment: None }),
        _ => None,
    }
}

/// A chain as a chain file.
pub fn export<R: Reader + ?Sized>(repo: &R, chain: &str) -> AppResult<String> {
    let c = resolve::chain(repo, chain)?;
    let view = repo
        .chain_view(c.id)?
        .ok_or_else(|| AppError::not_found(format!("chain {chain}")))?;
    let mut subtasks = HashMap::new();
    for t in &view.tickets {
        let texts: Vec<String> = repo.subtasks(t.id)?.into_iter().map(|s| s.text).collect();
        subtasks.insert(t.id, texts);
    }
    Ok(toknila_core::plan::export(&view, &subtasks))
}

/// Adds a dependency between two nodes of a chain. A Ready ticket that now waits again goes
/// back to Backlog when the chain settles.
pub fn add_edge<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    chain: &str,
    from: &str,
    to: &str,
    kind: EdgeKind,
) -> AppResult<ChainId> {
    change(uow, chain, |ctx| {
        let edge = Edge {
            chain_id: ctx.view.chain.id,
            from: resolve::node(&ctx.view, from)?,
            to: resolve::node(&ctx.view, to)?,
            kind,
        };
        // The graph rules (unknown node, cycle) answer first; a refusal drops the whole change.
        ctx.add_edge(edge)?;
        Ok(ctx.view.check_dependency_change(edge.to)?)
    })
}

/// Removes a dependency between two nodes of a chain.
pub fn remove_edge<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    chain: &str,
    from: &str,
    to: &str,
) -> AppResult<ChainId> {
    change(uow, chain, |ctx| {
        let (a, b) = (
            resolve::node(&ctx.view, from)?,
            resolve::node(&ctx.view, to)?,
        );
        ctx.view.check_dependency_change(b)?;
        ctx.remove_edge(a, b)
            .map(|_| ())
            .ok_or_else(|| AppError::not_found(format!("edge {from} -> {to}")))
    })
}

/// Runs `f` on a chain addressed by a human reference; returns the chain's id.
pub(crate) fn change<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    chain: &str,
    f: impl FnOnce(&mut ChainTx) -> AppResult<()>,
) -> AppResult<ChainId> {
    let c = resolve::chain(uow, chain)?;
    with_chain(uow, c.id, f)?;
    Ok(c.id)
}

/// Moves a card directly before or after another one of its chain: the dependencies around it
/// are rewired as core plans them, in one change. A card that has started cannot move, and
/// nothing can be put before work that has started.
pub fn move_ticket<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    chain: &str,
    ticket: &str,
    anchor: &str,
    place: toknila_core::graph::Place,
) -> AppResult<ChainId> {
    change(uow, chain, |ctx| {
        let (card, anchor) = (
            resolve::node(&ctx.view, ticket)?,
            resolve::node(&ctx.view, anchor)?,
        );
        let (toknila_core::NodeRef::Ticket(card), toknila_core::NodeRef::Ticket(anchor)) =
            (card, anchor)
        else {
            return Err(AppError::bad_request("only cards can be moved"));
        };
        let plan = ctx.view.plan_move(card, anchor, place)?;
        for e in plan.remove {
            ctx.remove_edge(e.from, e.to);
        }
        for e in plan.add {
            ctx.add_edge(e)?;
        }
        Ok(())
    })
}

/// Hands every card of the chain back to the automatic layout (clears the places a person put
/// them at). Cards that have none are left alone.
pub fn arrange<U: UnitOfWork + ?Sized>(uow: &mut U, chain: &str) -> AppResult<ChainId> {
    change(uow, chain, |ctx| {
        let placed: Vec<_> = ctx
            .view
            .tickets
            .iter()
            .filter(|t| t.pos.is_some())
            .map(|t| t.id)
            .collect();
        for id in placed {
            ctx.edit_ticket(id, |t| t.pos = None)?;
        }
        Ok(())
    })
}

/// What a person may set on a chain after it is made (a project takes the review fields in
/// [`crate::projects::PatchProject`]). A field left out is kept.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct ChainSettings {
    pub review_mode: Option<ReviewMode>,
    /// Reviewer agent profile; an empty text goes back to `<worker profile>-reviewer`.
    pub reviewer: Option<String>,
    /// Whether the chain starts its ready cards by itself. Turning it on starts the ready cards
    /// that have a free slot, so it is a person's act; the planner cannot change a chain's graph
    /// while it is on.
    pub autostart: Option<bool>,
}

/// Sets a chain's review mode, reviewer and autostart, journals it (`ChainUpdated`) and lets the
/// schedule run, since turning autostart on may start cards. `project` narrows the name lookup.
/// Returns the chain's id.
pub fn set_settings<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    chain: &str,
    project: Option<&str>,
    settings: ChainSettings,
) -> AppResult<ChainId> {
    let c = resolve::chain_in(uow, project, chain)?;
    with_chain(uow, c.id, |ctx| -> AppResult<()> {
        ctx.edit_chain(|c| {
            if let Some(m) = settings.review_mode {
                c.review_mode = Some(m);
            }
            if let Some(r) = settings.reviewer {
                c.reviewer = Some(r.trim().to_owned()).filter(|r| !r.is_empty());
            }
            if let Some(a) = settings.autostart {
                c.autostart = a;
            }
        });
        Ok(())
    })?;
    Ok(c.id)
}
