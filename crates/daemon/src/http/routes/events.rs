//! `/events` (the journal) and the agent event routes.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use toknila_app::{agent, resolve};
use toknila_core::Actor;
use toknila_core::agent::{AgentEvent, ReleaseEvent};

use super::{ApiResult, chain_write, load_ticket_detail};
use crate::http::dto::{ChainDetail, EventQuery, NewActivity, TicketDetail};
use crate::http::error::ErrorBody;
use crate::server::{AppState, Principal, now};
use crate::store::EventRecord;

#[utoipa::path(get, path = "/events", tag = "events", params(EventQuery),
    responses((status = 200, body = Vec<EventRecord>)))]
pub async fn list_events(
    State(state): State<AppState>,
    Query(q): Query<EventQuery>,
) -> ApiResult<Json<Vec<EventRecord>>> {
    let limit = q.limit.unwrap_or(1000).min(10_000);
    Ok(Json(state.store.events_since(q.since, limit).await?))
}

#[utoipa::path(post, path = "/tickets/{ticket}/events", tag = "agent", request_body = AgentEvent,
    params(("ticket" = String, Path, description = "Ticket id or key")),
    responses((status = 200, body = TicketDetail), (status = 400, body = ErrorBody), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn agent_event(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(ticket): Path<String>,
    Json(event): Json<AgentEvent>,
) -> ApiResult<Json<TicketDetail>> {
    let at = now();
    let (detail, _) = state
        .store
        .write(Actor::Agent, at, move |tx| -> ApiResult<TicketDetail> {
            let t = resolve::ticket(&*tx, &ticket)?;
            // An agent token acts on its own ticket only.
            principal.owns(&t)?;
            // The ticket itself, or the card it proposed.
            let answer = agent::ticket_event(tx, &t, event)?;
            load_ticket_detail(&tx.repo(), answer, at)
        })
        .await?;
    Ok(Json(detail))
}

#[utoipa::path(post, path = "/tickets/{ticket}/activity", tag = "agent", request_body = NewActivity,
    params(("ticket" = String, Path, description = "Ticket id or key")),
    responses((status = 204, description = "Recorded"), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// A sign of life from the ticket's agent (a hook fired). Recorded as the run's last activity;
/// nothing about the ticket changes.
pub async fn ticket_activity(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(ticket): Path<String>,
    Json(input): Json<NewActivity>,
) -> ApiResult<StatusCode> {
    let t = state
        .store
        .read(move |repo| Ok(resolve::ticket(repo, &ticket)))
        .await??;
    principal.owns(&t)?;
    state.runners.tokens().touch(t.id, now(), &input.signal);
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/releases/{release}/events", tag = "agent", request_body = ReleaseEvent,
    params(("release" = String, Path, description = "Release id or version")),
    responses((status = 200, body = ChainDetail), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn release_event(
    State(state): State<AppState>,
    Path(release): Path<String>,
    Json(event): Json<ReleaseEvent>,
) -> ApiResult<Json<ChainDetail>> {
    let detail = chain_write(&state, Actor::Agent, move |tx| {
        let r = resolve::release(&*tx, &release)?;
        agent::release_event(tx, &r, event)?;
        Ok(r.chain_id)
    })
    .await?;
    Ok(Json(detail))
}
