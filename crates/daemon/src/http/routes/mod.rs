//! HTTP handlers, one module per resource. Each handler parses the request, runs one use case
//! from `toknila-app` in one transaction and answers with the fresh state.

pub mod agents;
pub mod chains;
pub mod events;
pub mod inbox;
pub mod profiles;
pub mod projects;
pub mod releases;
pub mod runner;
pub mod system;
pub mod threads;
pub mod tickets;

use axum::Json;
use toknila_app::schedule;
use toknila_core::{Actor, ChainId, Thread, Ticket, TicketId, Timestamp};

use super::dto::{ChainDetail, ThreadDetail, TicketDetail};
use super::error::ApiError;
use crate::server::{AppState, now};
use crate::store::{Repo, WriteTx};

type ApiResult<T> = Result<T, ApiError>;

fn load_chain_detail(repo: &Repo<'_>, chain: ChainId, at: Timestamp) -> ApiResult<ChainDetail> {
    let view = repo
        .chain_view(chain)?
        .ok_or_else(|| ApiError::not_found(format!("chain {chain}")))?;
    Ok(ChainDetail::from_view(view, at))
}

fn load_ticket_detail(repo: &Repo<'_>, ticket: TicketId, at: Timestamp) -> ApiResult<TicketDetail> {
    let t = repo
        .ticket(ticket)?
        .ok_or_else(|| ApiError::not_found(format!("ticket {ticket}")))?;
    let chain = repo.chain(t.chain_id)?.map(|c| c.name).unwrap_or_default();
    let review = toknila_app::reviews::view(repo, &t)?;
    Ok(TicketDetail {
        review,
        allowed_actions: toknila_core::machine::TicketAction::allowed(&t),
        subtasks: repo.subtasks(t.id)?,
        comments: repo.comments(t.id)?,
        in_status_ms: at.millis_since(t.status_since),
        phase: toknila_core::thread::phase(&repo.threads(t.id)?),
        queue_position: schedule::queue_positions(repo)?.get(&t.id).copied(),
        ship: repo.ship(t.id)?,
        chain,
        ticket: t,
    })
}

fn load_thread_detail(repo: &Repo<'_>, thread: Thread) -> ApiResult<ThreadDetail> {
    let messages = repo.thread_messages(thread.id)?;
    let versions = repo.thread_versions(thread.id)?;
    Ok(ThreadDetail {
        changes_requested: toknila_core::thread::changes_requested(&thread, &versions, &messages),
        messages,
        versions,
        thread,
    })
}

/// Runs a read-only query on the writer's connection.
async fn read<T: Send + 'static>(
    state: &AppState,
    f: impl FnOnce(&Repo<'_>) -> ApiResult<T> + Send + 'static,
) -> ApiResult<T> {
    // The closure's ApiError rides inside an Ok.
    state.store.read(move |repo| Ok(f(repo))).await?
}

/// Runs `f` as one command of `actor` and answers with the chain it returns.
async fn chain_write(
    state: &AppState,
    actor: Actor,
    f: impl FnOnce(&mut WriteTx<'_>) -> ApiResult<ChainId> + Send + 'static,
) -> ApiResult<ChainDetail> {
    let at = now();
    let (detail, _) = state
        .store
        .write(actor, at, move |tx| -> ApiResult<ChainDetail> {
            let chain = f(tx)?;
            load_chain_detail(&tx.repo(), chain, at)
        })
        .await?;
    Ok(detail)
}

/// Runs `f` for one ticket in a transaction and answers with the ticket's fresh detail.
async fn ticket_write(
    state: &AppState,
    ticket: String,
    actor: Actor,
    f: impl FnOnce(&mut WriteTx<'_>, Ticket) -> ApiResult<()> + Send + 'static,
) -> ApiResult<Json<TicketDetail>> {
    let at = now();
    let (detail, _) = state
        .store
        .write(actor, at, move |tx| -> ApiResult<TicketDetail> {
            let t = toknila_app::resolve::ticket(&*tx, &ticket)?;
            let id = t.id;
            f(tx, t)?;
            load_ticket_detail(&tx.repo(), id, at)
        })
        .await?;
    Ok(Json(detail))
}
