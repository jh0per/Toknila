//! `/chains/{chain}/tickets`, `/chains/{chain}/wildcards`, `/tickets`.

use std::collections::HashMap;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use toknila_app::tickets::{self, TicketAction};
use toknila_app::{notes, resolve, schedule, wildcards};
use toknila_core::{Actor, ChainId};

use super::{ApiResult, load_ticket_detail, read, ticket_write};
use crate::http::dto::{
    ActionInput, NewComment, NewSubtask, NewTicket, NewWildcard, PatchSubtask, PatchTicket,
    TicketDetail, TicketItem, TicketQuery,
};
use crate::http::error::ErrorBody;
use crate::server::{AppState, now};

#[utoipa::path(post, path = "/chains/{chain}/tickets", tag = "tickets", request_body = NewTicket,
    params(("chain" = String, Path, description = "Chain id or name")),
    responses((status = 201, body = TicketDetail), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn create_ticket(
    State(state): State<AppState>,
    Path(chain): Path<String>,
    Json(input): Json<NewTicket>,
) -> ApiResult<(StatusCode, Json<TicketDetail>)> {
    let at = now();
    let (detail, _) = state
        .store
        .write(Actor::Human, at, move |tx| -> ApiResult<TicketDetail> {
            let id = tickets::create(tx, &chain, input)?;
            load_ticket_detail(&tx.repo(), id, at)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(detail)))
}

#[utoipa::path(post, path = "/chains/{chain}/wildcards", tag = "tickets", request_body = NewWildcard,
    params(("chain" = String, Path, description = "Chain id or name")),
    responses((status = 201, body = TicketDetail), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// A person proposes a wildcard card: it waits in Proposed until a person approves or rejects it.
pub async fn propose_wildcard(
    State(state): State<AppState>,
    Path(chain): Path<String>,
    Json(input): Json<NewWildcard>,
) -> ApiResult<(StatusCode, Json<TicketDetail>)> {
    let at = now();
    let (detail, _) = state
        .store
        .write(Actor::Human, at, move |tx| -> ApiResult<TicketDetail> {
            let id = wildcards::propose(tx, &chain, input.card, input.from.as_deref())?;
            load_ticket_detail(&tx.repo(), id, at)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(detail)))
}

#[utoipa::path(get, path = "/tickets", tag = "tickets", params(TicketQuery),
    responses((status = 200, body = Vec<TicketItem>)))]
pub async fn list_tickets(
    State(state): State<AppState>,
    Query(q): Query<TicketQuery>,
) -> ApiResult<Json<Vec<TicketItem>>> {
    let at = now();
    let items = read(&state, move |repo| {
        let chain = q
            .chain
            .as_deref()
            .map(|c| resolve::chain(repo, c))
            .transpose()?;
        let names: HashMap<ChainId, String> = repo
            .chains(None)?
            .into_iter()
            .map(|c| (c.id, c.name))
            .collect();
        let queue = schedule::queue_positions(repo)?;
        Ok(repo
            .tickets(chain.map(|c| c.id))?
            .into_iter()
            .filter(|t| !q.needs_human || t.status.needs_human())
            .map(|t| TicketItem {
                chain: names.get(&t.chain_id).cloned().unwrap_or_default(),
                in_status_ms: at.millis_since(t.status_since),
                needs_human: t.status.needs_human(),
                queue_position: queue.get(&t.id).copied(),
                ticket: t,
            })
            .collect())
    })
    .await?;
    Ok(Json(items))
}

#[utoipa::path(get, path = "/tickets/{ticket}", tag = "tickets",
    params(("ticket" = String, Path, description = "Ticket id or key, e.g. DEMO-3")),
    responses((status = 200, body = TicketDetail), (status = 404, body = ErrorBody)))]
pub async fn get_ticket(
    State(state): State<AppState>,
    Path(ticket): Path<String>,
) -> ApiResult<Json<TicketDetail>> {
    let at = now();
    Ok(Json(
        read(&state, move |repo| {
            let t = resolve::ticket(repo, &ticket)?;
            load_ticket_detail(repo, t.id, at)
        })
        .await?,
    ))
}

#[utoipa::path(patch, path = "/tickets/{ticket}", tag = "tickets", request_body = PatchTicket,
    params(("ticket" = String, Path, description = "Ticket id or key")),
    responses((status = 200, body = TicketDetail), (status = 404, body = ErrorBody)))]
pub async fn patch_ticket(
    State(state): State<AppState>,
    Path(ticket): Path<String>,
    Json(patch): Json<PatchTicket>,
) -> ApiResult<Json<TicketDetail>> {
    ticket_write(&state, ticket, Actor::Human, move |tx, t| {
        Ok(tickets::edit(tx, &t, patch)?)
    })
    .await
}

#[utoipa::path(post, path = "/tickets/{ticket}/actions/{action}", tag = "tickets", request_body = ActionInput,
    params(
        ("ticket" = String, Path, description = "Ticket id or key"),
        ("action" = String, Path, description = "start, pause, resume, block, unblock, accept, return, reopen, cancel, approve or reject"),
    ),
    responses((status = 200, body = TicketDetail), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn ticket_action(
    State(state): State<AppState>,
    Path((ticket, action)): Path<(String, String)>,
    body: Option<Json<ActionInput>>,
) -> ApiResult<Json<TicketDetail>> {
    let text = body.and_then(|Json(b)| b.text);
    let by = Actor::Human;
    let event = action.parse::<TicketAction>()?.event(text, by)?;
    ticket_write(&state, ticket, by, move |tx, t| {
        Ok(tickets::act(tx, &t, event)?)
    })
    .await
}

#[utoipa::path(post, path = "/tickets/{ticket}/comments", tag = "tickets", request_body = NewComment,
    params(("ticket" = String, Path, description = "Ticket id or key")),
    responses((status = 200, body = TicketDetail), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)))]
pub async fn add_comment(
    State(state): State<AppState>,
    Path(ticket): Path<String>,
    Json(input): Json<NewComment>,
) -> ApiResult<Json<TicketDetail>> {
    let (kind, body) = input.into_parts()?;
    ticket_write(&state, ticket, Actor::Human, move |tx, t| {
        Ok(tickets::comment(tx, &t, kind, body)?)
    })
    .await
}

#[utoipa::path(post, path = "/tickets/{ticket}/subtasks", tag = "tickets", request_body = NewSubtask,
    params(("ticket" = String, Path, description = "Ticket id or key")),
    responses((status = 200, body = TicketDetail), (status = 404, body = ErrorBody)))]
pub async fn add_subtask(
    State(state): State<AppState>,
    Path(ticket): Path<String>,
    Json(input): Json<NewSubtask>,
) -> ApiResult<Json<TicketDetail>> {
    ticket_write(&state, ticket, Actor::Human, move |tx, t| {
        notes::add_subtask(tx, &t, &input.text)?;
        Ok(())
    })
    .await
}

#[utoipa::path(patch, path = "/tickets/{ticket}/subtasks/{position}", tag = "tickets", request_body = PatchSubtask,
    params(("ticket" = String, Path, description = "Ticket id or key"), ("position" = u32, Path, description = "1-based position")),
    responses((status = 200, body = TicketDetail), (status = 404, body = ErrorBody)))]
pub async fn patch_subtask(
    State(state): State<AppState>,
    Path((ticket, position)): Path<(String, u32)>,
    Json(input): Json<PatchSubtask>,
) -> ApiResult<Json<TicketDetail>> {
    ticket_write(&state, ticket, Actor::Human, move |tx, t| {
        notes::update_subtask(tx, &t, position, input.done, input.text)?;
        Ok(())
    })
    .await
}
