//! `/planner/...`: what a project's planner agent does with its own token. The project is the
//! token's, so no path names one; every handler asks [`Principal::planner_project`] and the use
//! cases of `toknila_app::planner` keep each reference inside that project. These are the only
//! routes a planner's token reaches (`PLANNER_ROUTES`): nothing that starts, accepts or merges.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use toknila_api_types::{PlannerDesign, PlannerPlan};
use toknila_app::planner::{self, CardEdit, DesignPick, NewCard, NewPlanChain};
use toknila_app::threads::{self, NewMessage, NewThread, NewVersion};
use toknila_core::design::DesignRef;
use toknila_core::{Actor, ProjectId, ThreadId, Ticket};

use super::{ApiResult, chain_write, load_thread_detail, load_ticket_detail, read, stamp};
use crate::designs::Designs;
use crate::http::dto::{ChainDetail, EdgeInput, MoveInput, ThreadDetail, TicketDetail};
use crate::http::error::ErrorBody;
use crate::server::{AppState, Principal, now};
use crate::store::WriteTx;

const CHAIN: &str = "Chain id or name";
const CARD: &str = "Card id, key or title";
const THREAD: &str = "Thread number on the card (1, 2, ...) or thread id";

#[utoipa::path(get, path = "/planner/plan", tag = "planner",
    responses((status = 200, body = PlannerPlan), (status = 403, body = ErrorBody)))]
/// The whole plan of the planner's project: chains, cards in order, dependencies, threads, and
/// the project's designs with the path of each file.
pub async fn planner_plan(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
) -> ApiResult<Json<PlannerPlan>> {
    let project = principal.planner_project()?;
    let at = now();
    let plan = read(&state, move |repo| Ok(planner::plan(repo, project, at)?)).await?;
    let designs = Designs::new(&state.home, project);
    let designs = tokio::task::spawn_blocking(move || designs.for_planner()).await??;
    Ok(Json(PlannerPlan { plan, designs }))
}

#[utoipa::path(get, path = "/planner/designs/{design}", tag = "planner",
    params(("design" = String, Path, description = "Design id or name")),
    responses((status = 200, body = PlannerDesign), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// One design of the planner's project: its metadata and the path of its file.
pub async fn planner_design(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(design): Path<String>,
) -> ApiResult<Json<PlannerDesign>> {
    let project = principal.planner_project()?;
    let designs = Designs::new(&state.home, project);
    let found = tokio::task::spawn_blocking(move || designs.find_for_planner(&design)).await??;
    Ok(Json(found))
}

#[utoipa::path(post, path = "/planner/chains", tag = "planner", request_body = NewPlanChain,
    responses((status = 201, body = ChainDetail), (status = 400, body = ErrorBody), (status = 403, body = ErrorBody), (status = 409, body = ErrorBody)))]
/// A new chain in the planner's project. It never starts its cards by itself.
pub async fn planner_new_chain(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(input): Json<NewPlanChain>,
) -> ApiResult<(StatusCode, Json<ChainDetail>)> {
    let project = principal.planner_project()?;
    let detail = chain_write(&state, Actor::Agent, move |tx| {
        Ok(planner::new_chain(tx, project, input)?)
    })
    .await?;
    Ok((StatusCode::CREATED, Json(detail)))
}

/// The designs a planner named, found in its project's library (a design the project lacks is a
/// 400 that lists the ones it has).
async fn resolve_picks(
    state: &AppState,
    project: ProjectId,
    picks: Vec<DesignPick>,
) -> ApiResult<Vec<DesignRef>> {
    if picks.is_empty() {
        return Ok(Vec::new());
    }
    let designs = Designs::new(&state.home, project);
    Ok(tokio::task::spawn_blocking(move || designs.resolve(&picks)).await??)
}

#[utoipa::path(post, path = "/planner/chains/{chain}/cards", tag = "planner", request_body = NewCard,
    params(("chain" = String, Path, description = CHAIN)),
    responses((status = 201, body = TicketDetail), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// A new card in a chain of the planner's project.
pub async fn planner_add_card(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(chain): Path<String>,
    Json(input): Json<NewCard>,
) -> ApiResult<(StatusCode, Json<TicketDetail>)> {
    let project = principal.planner_project()?;
    let (at, prices) = stamp(&state);
    let designs = resolve_picks(&state, project, input.designs.clone()).await?;
    let (detail, _) = state
        .store
        .write(Actor::Agent, at, move |tx| -> ApiResult<TicketDetail> {
            let id = planner::add_card_with_designs(tx, project, &chain, input, designs)?;
            load_ticket_detail(&tx.repo(), id, at, &prices)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(detail)))
}

#[utoipa::path(patch, path = "/planner/cards/{card}", tag = "planner", request_body = CardEdit,
    params(("card" = String, Path, description = CARD)),
    responses((status = 200, body = TicketDetail), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// Edits a card of the planner's project that has not started.
pub async fn planner_edit_card(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(card): Path<String>,
    Json(edit): Json<CardEdit>,
) -> ApiResult<Json<TicketDetail>> {
    let project = principal.planner_project()?;
    let (at, prices) = stamp(&state);
    let designs = match edit.designs.clone() {
        Some(picks) => Some(resolve_picks(&state, project, picks).await?),
        None => None,
    };
    let (detail, _) = state
        .store
        .write(Actor::Agent, at, move |tx| -> ApiResult<TicketDetail> {
            let id = planner::edit_card_with_designs(tx, project, &card, edit, designs)?;
            load_ticket_detail(&tx.repo(), id, at, &prices)
        })
        .await?;
    Ok(Json(detail))
}

#[utoipa::path(post, path = "/planner/cards/{card}/show", tag = "planner",
    params(("card" = String, Path, description = CARD)),
    responses((status = 200, body = TicketDetail), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// Asks the window to show a card of the planner's project (a `window.show_card` event).
pub async fn planner_show_card(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(card): Path<String>,
) -> ApiResult<Json<TicketDetail>> {
    let project = principal.planner_project()?;
    let (at, prices) = stamp(&state);
    let (detail, _) = state
        .store
        .write(Actor::Agent, at, move |tx| -> ApiResult<TicketDetail> {
            let t = planner::show_card(tx, project, &card)?;
            load_ticket_detail(&tx.repo(), t.id, at, &prices)
        })
        .await?;
    Ok(Json(detail))
}

#[utoipa::path(post, path = "/planner/chains/{chain}/edges", tag = "planner", request_body = EdgeInput,
    params(("chain" = String, Path, description = CHAIN)),
    responses((status = 200, body = ChainDetail), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
/// Makes `to` wait for `from`.
pub async fn planner_add_dependency(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(chain): Path<String>,
    Json(e): Json<EdgeInput>,
) -> ApiResult<Json<ChainDetail>> {
    let project = principal.planner_project()?;
    let detail = chain_write(&state, Actor::Agent, move |tx| {
        Ok(planner::add_dependency(
            tx, project, &chain, &e.from, &e.to,
        )?)
    })
    .await?;
    Ok(Json(detail))
}

#[utoipa::path(delete, path = "/planner/chains/{chain}/edges", tag = "planner",
    params(("chain" = String, Path, description = CHAIN), EdgeInput),
    responses((status = 200, body = ChainDetail), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// Removes the wait of `to` for `from`.
pub async fn planner_remove_dependency(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(chain): Path<String>,
    Query(e): Query<EdgeInput>,
) -> ApiResult<Json<ChainDetail>> {
    let project = principal.planner_project()?;
    let detail = chain_write(&state, Actor::Agent, move |tx| {
        Ok(planner::remove_dependency(
            tx, project, &chain, &e.from, &e.to,
        )?)
    })
    .await?;
    Ok(Json(detail))
}

#[utoipa::path(post, path = "/planner/chains/{chain}/cards/{card}/move", tag = "planner", request_body = MoveInput,
    params(("chain" = String, Path, description = CHAIN), ("card" = String, Path, description = CARD)),
    responses((status = 200, body = ChainDetail), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
/// Puts a card directly before or after another; the dependencies around it are rewired.
pub async fn planner_move_card(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path((chain, card)): Path<(String, String)>,
    Json(m): Json<MoveInput>,
) -> ApiResult<Json<ChainDetail>> {
    let project = principal.planner_project()?;
    let detail = chain_write(&state, Actor::Agent, move |tx| {
        Ok(planner::move_card(
            tx, project, &chain, &card, &m.anchor, m.place,
        )?)
    })
    .await?;
    Ok(Json(detail))
}

#[utoipa::path(post, path = "/planner/cards/{card}/threads", tag = "planner", request_body = NewThread,
    params(("card" = String, Path, description = CARD)),
    responses((status = 201, body = ThreadDetail), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// Opens a thread on a card of the planner's project.
pub async fn planner_open_thread(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(card): Path<String>,
    Json(input): Json<NewThread>,
) -> ApiResult<(StatusCode, Json<ThreadDetail>)> {
    let project = principal.planner_project()?;
    let detail = thread_write(&state, move |tx| {
        let (t, th) = planner::open_thread(tx, project, &card, input.kind, &input.title)?;
        Ok((t, th.id))
    })
    .await?;
    Ok((StatusCode::CREATED, detail))
}

#[utoipa::path(post, path = "/planner/cards/{card}/threads/{thread}/messages", tag = "planner", request_body = NewMessage,
    params(("card" = String, Path, description = CARD), ("thread" = String, Path, description = THREAD)),
    responses((status = 200, body = ThreadDetail), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// Posts a message in a thread of a card of the planner's project.
pub async fn planner_thread_message(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path((card, thread)): Path<(String, String)>,
    Json(input): Json<NewMessage>,
) -> ApiResult<Json<ThreadDetail>> {
    let project = principal.planner_project()?;
    thread_write(&state, move |tx| {
        let (t, th) = planner::thread(&*tx, project, &card, &thread)?;
        threads::post_message(tx, &t, &th, &input.text)?;
        Ok((t, th.id))
    })
    .await
}

#[utoipa::path(post, path = "/planner/cards/{card}/threads/{thread}/versions", tag = "planner", request_body = NewVersion,
    params(("card" = String, Path, description = CARD), ("thread" = String, Path, description = THREAD)),
    responses((status = 200, body = ThreadDetail), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// Posts the next artifact version of a thread; an approved thread reopens.
pub async fn planner_thread_version(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path((card, thread)): Path<(String, String)>,
    Json(input): Json<NewVersion>,
) -> ApiResult<Json<ThreadDetail>> {
    let project = principal.planner_project()?;
    thread_write(&state, move |tx| {
        let (t, th) = planner::thread(&*tx, project, &card, &thread)?;
        threads::post_version(tx, &t, &th, input)?;
        Ok((t, th.id))
    })
    .await
}

/// Runs `f` as the planner (an agent) and answers with the fresh detail of the thread it returns
/// together with the card it is on.
async fn thread_write(
    state: &AppState,
    f: impl FnOnce(&mut WriteTx<'_>) -> ApiResult<(Ticket, ThreadId)> + Send + 'static,
) -> ApiResult<Json<ThreadDetail>> {
    let (detail, _) = state
        .store
        .write(Actor::Agent, now(), move |tx| -> ApiResult<ThreadDetail> {
            let (t, id) = f(tx)?;
            let thread = threads::find(&*tx, &t, &id.to_string())?;
            load_thread_detail(&tx.repo(), thread)
        })
        .await?;
    Ok(Json(detail))
}
