//! Chains: create, import and export chain files, edit dependencies.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use toknila_core::events::DomainEvent;
use toknila_core::{Chain, ChainId, Edge, EdgeKind, ReviewMode, TicketKey};

use crate::notes::add_subtask;
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
    };
    uow.insert_chain(&c)?;
    uow.emit(&DomainEvent::ChainCreated { chain: c.clone() })?;
    Ok(c.id)
}

/// Creates a chain from a chain file. Every node and edge goes through the same path as single
/// edits, then the chain settles once (autostart); the whole import lands or nothing does.
pub fn import<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: &str,
    toml: &str,
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
    let (view, subtasks) = plan.instantiate(p.id, uow.now(), || {
        keys.next().expect("one key was taken per ticket")
    });
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
    for (id, texts) in subtasks {
        let ticket = uow
            .ticket(id)?
            .ok_or_else(|| AppError::not_found("ticket"))?;
        for s in &texts {
            add_subtask(uow, &ticket, s)?;
        }
    }
    Ok(chain_id)
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
        Ok(ctx.add_edge(edge)?)
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
