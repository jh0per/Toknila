//! `/chains/{chain}/tickets`, `/chains/{chain}/wildcards`, `/tickets`.

use std::collections::HashMap;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use toknila_app::ships::GuardError;
use toknila_app::tickets::{self, TicketAction};
use toknila_app::{notes, resolve, schedule, ships, wildcards};
use toknila_core::guard::ShipProblem;
use toknila_core::machine::TicketEvent;
use toknila_core::{Actor, ChainId, ReviewTurn, TicketStatus};

use super::{ApiResult, load_ticket_detail, read, stamp, ticket_write};
use crate::http::dto::{
    ActionInput, NewComment, NewSubtask, NewTicket, NewWildcard, PatchSubtask, PatchTicket,
    TicketDetail, TicketItem, TicketQuery,
};
use crate::http::error::ErrorBody;
use crate::server::{AppState, now};

/// Where the check of `t` stands: queued behind others in the machine's `line`, else what the
/// checker last reported. `None` outside the check turn.
fn check_phase_of(
    t: &toknila_core::Ticket,
    line: &[toknila_core::TicketId],
) -> Option<toknila_core::CheckPhase> {
    if t.review.turn != Some(ReviewTurn::Check) {
        return None;
    }
    line.iter()
        .position(|c| *c == t.id)
        .and_then(|i| u32::try_from(i).ok())
        .filter(|p| *p > 0)
        .map(|place| toknila_core::CheckPhase::Queued { place })
        .or_else(|| t.review.check.clone())
}

/// Refuses design references to a design the project does not have.
async fn check_design_ids(
    state: &AppState,
    project: toknila_core::ProjectId,
    refs: Vec<toknila_core::design::DesignRef>,
) -> ApiResult<()> {
    let designs = crate::designs::Designs::new(&state.home, project);
    Ok(tokio::task::spawn_blocking(move || designs.check_refs(&refs)).await??)
}

#[utoipa::path(post, path = "/chains/{chain}/tickets", tag = "tickets", request_body = NewTicket,
    params(("chain" = String, Path, description = "Chain id or name")),
    responses((status = 201, body = TicketDetail), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn create_ticket(
    State(state): State<AppState>,
    Path(chain): Path<String>,
    Json(input): Json<NewTicket>,
) -> ApiResult<(StatusCode, Json<TicketDetail>)> {
    if !input.designs.is_empty() {
        let (name, refs) = (chain.clone(), input.designs.clone());
        let project = read(&state, move |repo| {
            Ok(resolve::chain(repo, &name)?.project_id)
        })
        .await?;
        check_design_ids(&state, project, refs).await?;
    }
    let (at, prices) = stamp(&state);
    let (detail, _) = state
        .store
        .write(Actor::Human, at, move |tx| -> ApiResult<TicketDetail> {
            let id = tickets::create(tx, &chain, input)?;
            load_ticket_detail(&tx.repo(), id, at, &prices)
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
    let (at, prices) = stamp(&state);
    let (detail, _) = state
        .store
        .write(Actor::Human, at, move |tx| -> ApiResult<TicketDetail> {
            let id = wildcards::propose(tx, &chain, input.card, input.from.as_deref())?;
            load_ticket_detail(&tx.repo(), id, at, &prices)
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
    let line = state.store.check_line().snapshot();
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
                check: check_phase_of(&t, &line),
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
    let (at, prices) = stamp(&state);
    Ok(Json(
        read(&state, move |repo| {
            let t = resolve::ticket(repo, &ticket)?;
            load_ticket_detail(repo, t.id, at, &prices)
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
    if let Some(refs) = patch.designs.clone() {
        let key = ticket.clone();
        let project = read(&state, move |repo| {
            let t = resolve::ticket(repo, &key)?;
            Ok(resolve::chain(repo, &t.chain_id.to_string())?.project_id)
        })
        .await?;
        check_design_ids(&state, project, refs).await?;
    }
    ticket_write(&state, ticket, Actor::Human, move |tx, t| {
        Ok(tickets::edit(tx, &t, patch)?)
    })
    .await
}

#[utoipa::path(post, path = "/tickets/{ticket}/actions/{action}", tag = "tickets", request_body = ActionInput,
    params(
        ("ticket" = String, Path, description = "Ticket id or key"),
        ("action" = String, Path, description = "start, pause, resume, block, unblock, accept, return, reopen, cancel, approve, reject or ask_design"),
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
    // Accept looks at the ship first, off the store thread: git never runs inside a transaction.
    if matches!(event, TicketEvent::Accept { .. }) {
        let looked = ticket.clone();
        let target = state
            .store
            .read(move |repo| {
                Ok(resolve::ticket(repo, &looked).and_then(|t| {
                    let waiting = t.status == TicketStatus::Review
                        && t.review.turn != Some(ReviewTurn::Merging);
                    let look = if waiting {
                        ships::finished_look(repo, &t)?
                    } else {
                        None
                    };
                    Ok(look)
                }))
            })
            .await??;
        if let Some(look) = target {
            // Only changes beyond the hand-in commit are refused here; an empty branch or a
            // ship git cannot look at is the merge's to block with its own reason.
            if let Err(e @ GuardError::Problem(ShipProblem::Uncommitted { .. })) =
                state.store.guard_ship(look.clone()).await
            {
                return Err(ships::refuse_accept(e, &look).into());
            }
        }
    }
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
