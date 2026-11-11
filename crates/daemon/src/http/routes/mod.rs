//! HTTP handlers, one module per resource. Each handler parses the request, runs one use case
//! from `toknila-app` in one transaction and answers with the fresh state.

pub mod agents;
pub mod chains;
pub mod designs;
pub mod events;
pub mod inbox;
pub mod pins;
pub mod planner;
pub mod planner_run;
pub mod plans;
pub mod profiles;
pub mod projects;
pub mod questions;
pub mod releases;
pub mod runner;
pub mod strays;
pub mod system;
pub mod threads;
pub mod tickets;
pub mod usage;

use axum::Json;
use axum::http::{HeaderMap, HeaderName, HeaderValue};
use axum::response::{IntoResponse, Response};
use toknila_api_types::runner::{HEADER_END, HEADER_OFFSET, HEADER_RUN};
use toknila_app::schedule;
use toknila_core::usage::{PriceTable, figures, figures_per_card};
use toknila_core::{Actor, ChainId, Thread, Ticket, TicketId, Timestamp};

use super::dto::{ChainDetail, ThreadDetail, TicketDetail};
use super::error::ApiError;
use crate::server::{AppState, now};
use crate::store::{Repo, RowFilter, WriteTx};
use toknila_app::AppError;

type ApiResult<T> = Result<T, ApiError>;

fn header(name: &'static str, value: impl ToString) -> (HeaderName, HeaderValue) {
    let value = HeaderValue::from_str(&value.to_string()).unwrap_or(HeaderValue::from_static(""));
    (HeaderName::from_static(name), value)
}

/// A terminal's scrollback as raw bytes, with the run and the offsets it covers in headers.
fn scrollback_response(chunk: crate::runner::Chunk) -> Response {
    let headers = HeaderMap::from_iter([
        header(HEADER_RUN, chunk.run),
        header(HEADER_OFFSET, chunk.offset),
        header(HEADER_END, chunk.end),
    ]);
    (headers, chunk.bytes).into_response()
}

/// What an answer is costed with, and the machine's line of checks as it stood when the command
/// began (a card's place in it is part of its detail). Reads as the price table.
struct Pricing {
    prices: PriceTable,
    checks: Vec<TicketId>,
}

impl std::ops::Deref for Pricing {
    type Target = PriceTable;
    fn deref(&self) -> &PriceTable {
        &self.prices
    }
}

/// The time of a command, and the prices its answer is costed with.
fn stamp(state: &AppState) -> (Timestamp, Pricing) {
    let checks = state.store.check_line().snapshot();
    (
        now(),
        Pricing {
            prices: state.usage.prices().0,
            checks,
        },
    )
}

fn load_chain_detail(
    repo: &Repo<'_>,
    chain: ChainId,
    at: Timestamp,
    prices: &PriceTable,
) -> ApiResult<ChainDetail> {
    let view = repo
        .chain_view(chain)?
        .ok_or_else(|| AppError::not_found(format!("chain {chain}")))?;
    let rows = repo.usage_rows(RowFilter {
        chain: Some(chain),
        ..RowFilter::default()
    })?;
    let mut detail = ChainDetail::from_view(view, at);
    detail.usage = figures_per_card(&rows, prices);
    Ok(detail)
}

fn load_ticket_detail(
    repo: &Repo<'_>,
    ticket: TicketId,
    at: Timestamp,
    prices: &Pricing,
) -> ApiResult<TicketDetail> {
    let t = repo
        .ticket(ticket)?
        .ok_or_else(|| AppError::not_found(format!("ticket {ticket}")))?;
    let chain = repo.chain(t.chain_id)?.map(|c| c.name).unwrap_or_default();
    let mut review = toknila_app::reviews::view(repo, &t)?;
    if review.awaiting == Some(toknila_core::ReviewTurn::Check) {
        review.check_queue = prices
            .checks
            .iter()
            .position(|c| *c == t.id)
            .and_then(|i| u32::try_from(i).ok());
        if let Some(place) = review.check_queue.filter(|p| *p > 0) {
            review.check = Some(toknila_core::CheckPhase::Queued { place });
        }
    }
    let rows = repo.usage_rows(RowFilter {
        ticket: Some(t.id),
        ..RowFilter::default()
    })?;
    let threads = repo.threads(t.id)?;
    let mut design_waiting = false;
    for th in &threads {
        let (versions, messages) = (repo.thread_versions(th.id)?, repo.thread_messages(th.id)?);
        design_waiting |= toknila_core::thread::awaiting_person(th, &versions, &messages).is_some();
    }
    Ok(TicketDetail {
        design_waiting,
        usage: figures(&rows, prices),
        review,
        allowed_actions: toknila_core::machine::TicketAction::allowed(&t),
        can_edit: t.status.can_edit(),
        can_move: t.status.can_move(),
        subtasks: repo.subtasks(t.id)?,
        comments: repo.comments(t.id)?,
        in_status_ms: at.millis_since(t.status_since),
        phase: toknila_core::thread::phase(&threads),
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
    let (at, prices) = stamp(state);
    let (detail, _) = state
        .store
        .write(actor, at, move |tx| -> ApiResult<ChainDetail> {
            let chain = f(tx)?;
            load_chain_detail(&tx.repo(), chain, at, &prices)
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
    let (at, prices) = stamp(state);
    let (detail, _) = state
        .store
        .write(actor, at, move |tx| -> ApiResult<TicketDetail> {
            let t = toknila_app::resolve::ticket(&*tx, &ticket)?;
            let id = t.id;
            f(tx, t)?;
            load_ticket_detail(&tx.repo(), id, at, &prices)
        })
        .await?;
    Ok(Json(detail))
}
