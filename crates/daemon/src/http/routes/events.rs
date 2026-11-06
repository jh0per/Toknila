//! `/events` (the journal) and the agent event routes.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use toknila_app::{agent, instructions, resolve, ships};
use toknila_core::agent::{AgentEvent, ReleaseEvent};
use toknila_core::{Actor, TicketStatus, nudge};

use super::{ApiResult, chain_write, load_ticket_detail, stamp};
use crate::http::dto::{ChainDetail, EventQuery, InstructionClaim, NewActivity, TicketDetail};
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
    let (at, prices) = stamp(&state);
    // The ship is looked at here, off the store thread and before the command: git never runs
    // inside a transaction. The card stays Running when the work is not in commits.
    if matches!(event, AgentEvent::Finished { .. }) {
        let looked = ticket.clone();
        let target = state
            .store
            .read(move |repo| {
                Ok(resolve::ticket(repo, &looked).and_then(|t| {
                    let look = ships::finished_look(repo, &t)?;
                    Ok((t, look))
                }))
            })
            .await??;
        principal.owns(&target.0)?;
        if let Some(look) = target.1 {
            state
                .store
                .guard_ship(look.clone())
                .await
                .map_err(|e| ships::refuse_finished(e, &look))?;
        }
    }
    let (detail, _) = state
        .store
        .write(Actor::Agent, at, move |tx| -> ApiResult<TicketDetail> {
            let t = resolve::ticket(&*tx, &ticket)?;
            // An agent token acts on its own ticket only, and says what its role may.
            principal.owns(&t)?;
            principal.may_send(&event)?;
            // The ticket itself, or the card it proposed.
            let answer = agent::ticket_event(tx, &t, event)?;
            load_ticket_detail(&tx.repo(), answer, at, &prices)
        })
        .await?;
    Ok(Json(detail))
}

#[utoipa::path(post, path = "/tickets/{ticket}/activity", tag = "agent", request_body = NewActivity,
    params(("ticket" = String, Path, description = "Ticket id or key")),
    responses((status = 204, description = "Recorded"), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// A sign of life from the ticket's agent (a hook fired). Recorded as the run's last activity;
/// nothing about the ticket changes unless the signal shows work and the ticket waits.
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
    // Which Claude Code session this run is, whoever the agent: worker or reviewer.
    if let (Some(session), Some(transcript)) = (&input.session_id, &input.transcript_path) {
        let run = match principal {
            Principal::Agent(g) => Some((g.run, g.role)),
            Principal::Person => None,
        };
        let signal = &input.signal;
        if let Err(error) = state
            .usage
            .attribute(&state.store, t.id, run, (session, transcript), signal)
            .await
        {
            tracing::warn!(%error, ticket = %t.key, "could not read the session's usage");
        }
    }
    // Only the worker's signs of life count: a reviewer beside it is not the worker working, and
    // never brings a waiting worker back to Running.
    if !matches!(principal, Principal::Agent(g) if g.role != toknila_core::AgentRole::Worker) {
        let at = now();
        state.runners.tokens().touch(t.id, at, &input.signal);
        if t.status == TicketStatus::Waiting && nudge::resumes(&input.signal) {
            let signal = input.signal;
            state
                .store
                .write(Actor::Agent, at, move |tx| -> ApiResult<()> {
                    Ok(agent::activity(tx, &t, &signal)?)
                })
                .await?;
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/tickets/{ticket}/instructions/claim", tag = "agent",
    params(("ticket" = String, Path, description = "Ticket id or key")),
    responses((status = 200, body = InstructionClaim), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// The ticket's agent asks, from its Stop hook, for what a person told it while it worked. Each
/// instruction is handed out once: it is marked delivered as it is claimed.
pub async fn claim_instructions(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(ticket): Path<String>,
) -> ApiResult<Json<InstructionClaim>> {
    let at = now();
    let (claimed, _) = state
        .store
        .write(
            Actor::Agent,
            at,
            move |tx| -> ApiResult<instructions::Claimed> {
                let t = resolve::ticket(&*tx, &ticket)?;
                principal.owns(&t)?;
                Ok(instructions::claim(tx, &t)?)
            },
        )
        .await?;
    Ok(Json(InstructionClaim {
        count: claimed.count,
        reason: claimed.reason,
    }))
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
