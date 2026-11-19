//! `/chains/{chain}/tickets`, `/chains/{chain}/wildcards`, `/tickets`. The routes a planner
//! reaches too (`PLANNER_ROUTES`) run as the caller the token names; core's `access::may`
//! decides what it may.

use std::collections::HashMap;

use crate::http::body::Json;
use crate::http::query::Query;
use axum::Extension;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use toknila_app::access::{self, Action, Caller};
use toknila_app::ships::GuardError;
use toknila_app::tickets::{self, TicketAction};
use toknila_app::{as_caller, moves, notes, resolve, schedule, ships, wildcards};
use toknila_core::design::DesignPick;
use toknila_core::guard::ShipProblem;
use toknila_core::machine::TicketEvent;
use toknila_core::{Actor, ChainId, ReviewTurn, TicketStatus};

use super::{ApiResult, load_ticket_detail, read, stamp, ticket_write};
use crate::http::dto::{
    ActionInput, DesignMoveInput, MoveFrom, MoveToChain, NewComment, NewSubtask, NewTicket,
    NewWildcard, PatchSubtask, PatchTicket, TicketDetail, TicketItem, TicketQuery, clamp_size,
};
use crate::http::error::ErrorBody;
use crate::server::{AppState, Principal, now};

/// Where the check of `t` stands: queued behind others in the machine's `line`, else what the
/// checker last reported. `None` outside the check turn.
fn check_phase_of(
    t: &toknila_core::Ticket,
    line: &toknila_app::checks::Standing,
) -> Option<toknila_core::CheckPhase> {
    if t.review.turn != Some(ReviewTurn::Check) {
        return None;
    }
    line.place(t.id)
        .filter(|(place, _)| *place > 0)
        .map(|(place, held)| toknila_core::CheckPhase::Queued { place, held })
        .or_else(|| t.review.check.clone())
}

/// `picks` with every design named by its id in `project`'s library: one the project does not
/// have is refused, with the names it has.
async fn by_id(
    state: &AppState,
    project: toknila_core::ProjectId,
    picks: Vec<DesignPick>,
) -> ApiResult<Vec<DesignPick>> {
    if picks.is_empty() {
        return Ok(picks);
    }
    let designs = crate::designs::Designs::new(&state.home, project);
    let refs = tokio::task::spawn_blocking(move || designs.resolve(&picks)).await??;
    Ok(refs.into_iter().map(DesignPick::from).collect())
}

/// `caller` adds a story to `chain`; its designs may be named by file name.
pub(super) async fn add_story(
    state: &AppState,
    caller: Caller,
    chain: String,
    mut input: NewTicket,
) -> ApiResult<TicketDetail> {
    if !input.designs.is_empty() {
        let name = chain.clone();
        // The epic is the caller's to read before its project's library is looked at.
        let project = read(state, move |repo| {
            let c = access::chain(repo, &caller, None, &name)?;
            access::on_epic(repo, &caller, &Action::ReadPlan, &c)?;
            Ok(c.project_id)
        })
        .await?;
        input.designs = by_id(state, project, std::mem::take(&mut input.designs)).await?;
    }
    let (at, prices) = stamp(state);
    let (detail, _) = state
        .store
        .write(caller.actor(), at, move |tx| -> ApiResult<TicketDetail> {
            let id = as_caller::add_story(tx, &caller, &chain, input)?;
            load_ticket_detail(&tx.repo(), id, at, &prices)
        })
        .await?;
    Ok(detail)
}

/// `caller` edits a story; its designs may be named by file name, and a size out of range is
/// kept at the nearest size inside the limits (the answer says so).
pub(super) async fn edit_story(
    state: &AppState,
    caller: Caller,
    ticket: String,
    mut patch: PatchTicket,
) -> ApiResult<TicketDetail> {
    let mut clamped = None;
    if let Some(Some(asked)) = patch.size {
        let kept = clamp_size(asked);
        if kept != asked {
            clamped = Some(asked);
            patch.size = Some(Some(kept));
        }
    }
    if let Some(picks) = patch.designs.take() {
        let key = ticket.clone();
        let project = read(state, move |repo| {
            let t = resolve::ticket(repo, &key)?;
            access::on_story(repo, &caller, &Action::ReadPlan, &t)?;
            Ok(resolve::chain(repo, &t.chain_id.to_string())?.project_id)
        })
        .await?;
        patch.designs = Some(by_id(state, project, picks).await?);
    }
    let Json(mut detail) = ticket_write(state, ticket, caller.actor(), move |tx, t| {
        as_caller::edit_story(tx, &caller, &t, patch)?;
        Ok(())
    })
    .await?;
    detail.size_clamped = clamped;
    Ok(detail)
}

#[utoipa::path(post, path = "/chains/{chain}/tickets", tag = "tickets", request_body = NewTicket,
    params(("chain" = String, Path, description = "Chain id or name")),
    responses((status = 201, body = TicketDetail), (status = 400, body = ErrorBody), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
/// A new story in an epic. A planner adds only its fields, to an epic that waits for the owner to
/// start its stories.
pub async fn create_ticket(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(chain): Path<String>,
    Json(input): Json<NewTicket>,
) -> ApiResult<(StatusCode, Json<TicketDetail>)> {
    let detail = add_story(&state, principal.caller(), chain, input).await?;
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
            let (card, from) = input.into_parts();
            let id = wildcards::propose(tx, &chain, card, from.as_deref())?;
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
    let line = state.store.check_line().standing();
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
        let queue = schedule::queue(repo)?;
        let projects: HashMap<_, _> = repo.projects()?.into_iter().map(|p| (p.id, p)).collect();
        let owners: HashMap<ChainId, _> = repo
            .chains(None)?
            .into_iter()
            .map(|c| (c.id, c.project_id))
            .collect();
        // A project's merge line is read once, for the first card that waits in it.
        let mut merge_lines = HashMap::new();
        let mut items = Vec::new();
        let tickets = repo.tickets(chain.map(|c| c.id))?.into_iter();
        for t in tickets.filter(|t| !q.needs_human || t.status.needs_human()) {
            let project = owners.get(&t.chain_id).copied();
            let merge_wait = match (t.review.merge_line, project) {
                (Some(place), Some(p)) if !place.holds => {
                    let line = match merge_lines.entry(p) {
                        std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                        std::collections::hash_map::Entry::Vacant(e) => {
                            e.insert(toknila_app::merges::line_of(repo, p)?)
                        }
                    };
                    line.card_wait(t.id)
                }
                _ => None,
            };
            items.push(TicketItem {
                simulated: project
                    .and_then(|p| projects.get(&p))
                    .is_some_and(|p| p.runs_simulated(&t)),
                chain: names.get(&t.chain_id).cloned().unwrap_or_default(),
                in_status_ms: at.millis_since(t.status_since),
                needs_human: t.status.needs_human(),
                queue_position: queue.get(&t.id).map(|(n, _)| *n),
                queue_text: queue.get(&t.id).map(|(_, text)| text.clone()),
                check: check_phase_of(&t, &line),
                merge_wait,
                ticket: t,
            });
        }
        Ok(items)
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
    responses((status = 200, body = TicketDetail), (status = 400, body = ErrorBody), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
/// Changes a story's fields, and renames and appends checklist items, in one command. A planner
/// edits a story that has not started, and only its words, checklist, designs and model.
pub async fn patch_ticket(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(ticket): Path<String>,
    Json(patch): Json<PatchTicket>,
) -> ApiResult<Json<TicketDetail>> {
    Ok(Json(
        edit_story(&state, principal.caller(), ticket, patch).await?,
    ))
}

#[utoipa::path(post, path = "/tickets/{ticket}/designs/{index}/move", tag = "tickets", request_body = DesignMoveInput,
    params(
        ("ticket" = String, Path, description = "Ticket id or key that holds the reference"),
        ("index" = usize, Path, description = "Position of the reference in the card's list"),
    ),
    responses((status = 200, body = TicketDetail), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
/// Gives a design reference, region kept, to another card of the chain in one transaction.
pub async fn move_design(
    State(state): State<AppState>,
    Path((ticket, index)): Path<(String, usize)>,
    Json(m): Json<DesignMoveInput>,
) -> ApiResult<Json<TicketDetail>> {
    ticket_write(&state, ticket, Actor::Human, move |tx, t| {
        Ok(moves::move_design(tx, &t, index, &m.target)?)
    })
    .await
}

#[utoipa::path(post, path = "/tickets/{ticket}/move-to-chain", tag = "tickets", request_body = MoveToChain,
    params(("ticket" = String, Path, description = "Ticket id or key"), MoveFrom),
    responses((status = 200, body = TicketDetail), (status = 400, body = ErrorBody), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
/// Moves a card that has not started to another epic of its project, to the end or next to a card.
/// When a planner moves it, neither epic may start its stories by itself.
pub async fn move_to_chain(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(ticket): Path<String>,
    Query(q): Query<MoveFrom>,
    Json(m): Json<MoveToChain>,
) -> ApiResult<Json<TicketDetail>> {
    let caller = principal.caller();
    ticket_write(&state, ticket, caller.actor(), move |tx, t| {
        let place = m.place.as_ref().map(|p| (p.anchor.as_str(), p.place));
        let (story, from) = (t.id.to_string(), q.from.as_deref());
        as_caller::move_story_to_epic(tx, &caller, &story, &m.chain, place, from)?;
        Ok(())
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
                    Ok(look.map(|l| (l, t.review.clone())))
                }))
            })
            .await??;
        if let Some((look, review)) = target {
            // Only changes beyond the hand-in commit are refused here; an empty branch or a
            // ship git cannot look at is the merge's to block with its own reason.
            if let Err(e @ GuardError::Problem(ShipProblem::Uncommitted { .. })) =
                state.store.guard_ship(look.clone()).await
            {
                return Err(ships::refuse_accept(e, &look).into());
            }
            // Nothing lands that was not handed in: commits beyond the hand-in, other than the
            // daemon's own base-sync merges, are refused. A ship git cannot look at is the
            // merge's to block.
            if let Some(handed_in) = review.handed_in.clone()
                && let Ok(extra) = state.store.extra_commits(look.clone(), review).await
                && !extra.is_empty()
            {
                return Err(toknila_app::hand_in::refuse_extra(&extra, &handed_in).into());
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
