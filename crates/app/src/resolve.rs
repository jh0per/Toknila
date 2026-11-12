//! References: humans address things by id, key, name, version or (unique) title.

use toknila_core::graph::ChainView;
use toknila_core::{Chain, NodeRef, Project, Release, Ticket, TicketKey};

use crate::{AppError, AppResult, Reader};

/// A project by id or key (any case).
pub fn project<R: Reader + ?Sized>(repo: &R, r: &str) -> AppResult<Project> {
    if let Ok(id) = r.parse()
        && let Some(p) = repo.project(id)?
    {
        return Ok(p);
    }
    repo.project_by_key(&r.to_uppercase())?
        .ok_or_else(|| AppError::not_found(format!("project {r}")))
}

/// A chain by id or unique name.
pub fn chain<R: Reader + ?Sized>(repo: &R, r: &str) -> AppResult<Chain> {
    if let Ok(id) = r.parse()
        && let Some(c) = repo.chain(id)?
    {
        return Ok(c);
    }
    let found = repo.chains_named(r)?;
    unique(
        found,
        || format!("epic {r}"),
        |n| format!("{n} epics are named {r}; use the epic id"),
    )
}

/// A chain by id or name, looking only at the chains of `project` (id or key) when it is given:
/// two projects may have a chain of the same name.
pub fn chain_in<R: Reader + ?Sized>(repo: &R, project: Option<&str>, r: &str) -> AppResult<Chain> {
    let Some(project) = project else {
        return chain(repo, r);
    };
    let p = self::project(repo, project)?;
    let mut found = repo.chains(Some(p.id))?;
    found.retain(|c| c.id.to_string() == r || c.name == r);
    unique(
        found,
        || format!("epic {r} in project {}", p.key),
        |n| format!("{n} epics are named {r}; use the epic id"),
    )
}

/// A ticket by id, key (any case) or unique title.
pub fn ticket<R: Reader + ?Sized>(repo: &R, r: &str) -> AppResult<Ticket> {
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
    let found = repo
        .tickets(None)?
        .into_iter()
        .filter(|t| t.title == r)
        .collect();
    unique(
        found,
        || format!("ticket {r}"),
        |n| format!("{n} tickets are titled {r:?}; use the key"),
    )
}

/// A milestone by id or unique version.
pub fn release<R: Reader + ?Sized>(repo: &R, r: &str) -> AppResult<Release> {
    if let Ok(id) = r.parse()
        && let Some(rel) = repo.release(id)?
    {
        return Ok(rel);
    }
    let found = repo.releases_by_version(r)?;
    unique(
        found,
        || format!("release {r}"),
        |n| format!("{n} releases have version {r}; use the release id"),
    )
}

/// A node of one chain by ticket key, id or unique title, or by milestone version or id.
pub fn node(view: &ChainView, r: &str) -> AppResult<NodeRef> {
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
    Err(AppError::not_found(format!(
        "{r} in epic {}",
        view.chain.name
    )))
}

fn unique<T>(
    mut found: Vec<T>,
    what: impl FnOnce() -> String,
    many: impl FnOnce(usize) -> String,
) -> AppResult<T> {
    match found.len() {
        0 => Err(AppError::not_found(what())),
        1 => Ok(found.remove(0)),
        n => Err(AppError::ambiguous(many(n))),
    }
}
