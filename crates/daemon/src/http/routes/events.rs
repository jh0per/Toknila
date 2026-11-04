//! `/events` (the journal) and the agent event routes.

use axum::Json;
use axum::extract::{Path, Query, State};
use toknila_app::{agent, resolve};
use toknila_core::Actor;
use toknila_core::agent::{AgentEvent, ReleaseEvent};

use super::{ApiResult, chain_write, ticket_write};
use crate::http::dto::{ChainDetail, EventQuery, TicketDetail};
use crate::http::error::ErrorBody;
use crate::server::AppState;
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
    responses((status = 200, body = TicketDetail), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn agent_event(
    State(state): State<AppState>,
    Path(ticket): Path<String>,
    Json(event): Json<AgentEvent>,
) -> ApiResult<Json<TicketDetail>> {
    ticket_write(&state, ticket, Actor::Agent, move |tx, t| {
        Ok(agent::ticket_event(tx, &t, event)?)
    })
    .await
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
