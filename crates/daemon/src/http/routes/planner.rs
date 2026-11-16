//! `/planner/...`: the planner's own routes before TOKNILA-149, kept for one release as thin
//! aliases, `deprecated` in the OpenAPI document. Each takes the project from the planner's token
//! (a person's token is refused here, `403`, as before), turns its body into the person's and
//! calls what the person's route calls, so it answers the same; core's `access::may` decides what
//! the planner may. The planner's tools call the person's routes; `say` and `activity` stay
//! planner routes (`conversation.rs`, `planner_activity.rs`), since no person route does those.

#![allow(deprecated)]

use crate::http::body::Json;
use crate::http::query::Query;
use axum::Extension;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use toknila_api_types::{PlanCard, PlanChain, PlannerDesign, PlannerOverview, PlannerPlan};
use toknila_app::planner::{self, CardEdit, EditPlanChain, NewCard, NewPlanChain, NewSuggestion};
use toknila_app::threads::{NewMessage, NewThread, NewVersion};
use toknila_core::EdgeKind;
use toknila_core::suggestion::Suggestion;

use super::{ApiResult, chain_write, chains, inbox, plan_view, threads, tickets};
use crate::http::dto::{ChainDetail, EdgeInput, PlannerMove, ThreadDetail, TicketDetail};
use crate::http::error::ErrorBody;
use crate::server::{AppState, Principal};

const CHAIN: &str = "Chain id or name";
const CARD: &str = "Card id, key or title";
const THREAD: &str = "Thread number on the card (1, 2, ...) or thread id";

#[utoipa::path(get, path = "/planner/plan", tag = "planner",
    responses((status = 200, body = PlannerPlan), (status = 403, body = ErrorBody)))]
#[deprecated(note = "GET /projects/{project}/plan; kept for one release")]
/// Deprecated alias of `GET /projects/{project}/plan` for the planner's own project.
pub async fn planner_plan(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
) -> ApiResult<Json<PlannerPlan>> {
    let project = principal.planner_project()?.to_string();
    let plan = plan_view::plan_of(&state, principal.caller(), project).await?;
    Ok(Json(plan))
}

#[utoipa::path(get, path = "/planner/overview", tag = "planner",
    responses((status = 200, body = PlannerOverview), (status = 403, body = ErrorBody)))]
#[deprecated(note = "GET /projects/{project}/plan/overview; kept for one release")]
/// Deprecated alias of `GET /projects/{project}/plan/overview` for the planner's own project.
pub async fn planner_overview(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
) -> ApiResult<Json<PlannerOverview>> {
    let project = principal.planner_project()?.to_string();
    let overview = plan_view::overview_of(&state, principal.caller(), project).await?;
    Ok(Json(overview))
}

#[utoipa::path(get, path = "/planner/chains/{chain}", tag = "planner",
    params(("chain" = String, Path, description = CHAIN)),
    responses((status = 200, body = PlanChain), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
#[deprecated(note = "GET /chains/{chain}/plan; kept for one release")]
/// Deprecated alias of `GET /chains/{chain}/plan`.
pub async fn planner_chain(
    state: State<AppState>,
    Extension(principal): Extension<Principal>,
    chain: Path<String>,
) -> ApiResult<Json<PlanChain>> {
    principal.planner_project()?;
    plan_view::chain_plan(state, Extension(principal), chain).await
}

#[utoipa::path(get, path = "/planner/cards/{card}", tag = "planner",
    params(("card" = String, Path, description = CARD)),
    responses((status = 200, body = PlanCard), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
#[deprecated(note = "GET /tickets/{ticket}/plan; kept for one release")]
/// Deprecated alias of `GET /tickets/{ticket}/plan`.
pub async fn planner_card(
    state: State<AppState>,
    Extension(principal): Extension<Principal>,
    card: Path<String>,
) -> ApiResult<Json<PlanCard>> {
    principal.planner_project()?;
    plan_view::ticket_plan(state, Extension(principal), card).await
}

#[utoipa::path(get, path = "/planner/designs/{design}", tag = "planner",
    params(("design" = String, Path, description = "Design id or name")),
    responses((status = 200, body = PlannerDesign), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
#[deprecated(note = "GET /projects/{project}/designs/{design}; kept for one release")]
/// Deprecated alias of `GET /projects/{project}/designs/{design}` for the planner's project.
pub async fn planner_design(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(design): Path<String>,
) -> ApiResult<Json<PlannerDesign>> {
    let project = principal.planner_project()?.to_string();
    let found = plan_view::design_of(&state, principal.caller(), project, design).await?;
    Ok(Json(found))
}

#[utoipa::path(post, path = "/planner/chains", tag = "planner", request_body = NewPlanChain,
    responses((status = 201, body = ChainDetail), (status = 400, body = ErrorBody), (status = 403, body = ErrorBody), (status = 409, body = ErrorBody)))]
#[deprecated(note = "POST /projects/{project}/chains; kept for one release")]
/// Deprecated alias of `POST /projects/{project}/chains` for the planner's project.
pub async fn planner_new_chain(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(input): Json<NewPlanChain>,
) -> ApiResult<(StatusCode, Json<ChainDetail>)> {
    let project = principal.planner_project()?.to_string();
    let detail = chains::new_epic(&state, principal.caller(), project, input.into()).await?;
    Ok((StatusCode::CREATED, Json(detail)))
}

#[utoipa::path(patch, path = "/planner/chains/{chain}", tag = "planner", request_body = EditPlanChain,
    params(("chain" = String, Path, description = CHAIN)),
    responses((status = 200, body = ChainDetail), (status = 400, body = ErrorBody), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
#[deprecated(note = "PATCH /chains/{chain}; kept for one release")]
/// Deprecated alias of `PATCH /chains/{chain}`.
pub async fn planner_edit_chain(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(chain): Path<String>,
    Json(edit): Json<EditPlanChain>,
) -> ApiResult<Json<ChainDetail>> {
    principal.planner_project()?;
    let detail = chains::edit_epic(&state, principal.caller(), chain, None, edit.into()).await?;
    Ok(Json(detail))
}

#[utoipa::path(post, path = "/planner/chains/{chain}/cards", tag = "planner", request_body = NewCard,
    params(("chain" = String, Path, description = CHAIN)),
    responses((status = 201, body = TicketDetail), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
#[deprecated(note = "POST /chains/{chain}/tickets; kept for one release")]
/// Deprecated alias of `POST /chains/{chain}/tickets`.
pub async fn planner_add_card(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(chain): Path<String>,
    Json(input): Json<NewCard>,
) -> ApiResult<(StatusCode, Json<TicketDetail>)> {
    principal.planner_project()?;
    let story = input.into_story();
    let detail = tickets::add_story(&state, principal.caller(), chain, story).await?;
    Ok((StatusCode::CREATED, Json(detail)))
}

#[utoipa::path(patch, path = "/planner/cards/{card}", tag = "planner", request_body = CardEdit,
    params(("card" = String, Path, description = CARD)),
    responses((status = 200, body = TicketDetail), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
#[deprecated(note = "PATCH /tickets/{ticket}; kept for one release")]
/// Deprecated alias of `PATCH /tickets/{ticket}`.
pub async fn planner_edit_card(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(card): Path<String>,
    Json(edit): Json<CardEdit>,
) -> ApiResult<Json<TicketDetail>> {
    principal.planner_project()?;
    let patch = edit.into_story();
    let detail = tickets::edit_story(&state, principal.caller(), card, patch).await?;
    Ok(Json(detail))
}

#[utoipa::path(post, path = "/planner/cards/{card}/show", tag = "planner",
    params(("card" = String, Path, description = CARD)),
    responses((status = 200, body = TicketDetail), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
#[deprecated(note = "POST /tickets/{ticket}/show; kept for one release")]
/// Deprecated alias of `POST /tickets/{ticket}/show`.
pub async fn planner_show_card(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(card): Path<String>,
) -> ApiResult<Json<TicketDetail>> {
    principal.planner_project()?;
    Ok(Json(
        plan_view::show(&state, principal.caller(), card).await?,
    ))
}

#[utoipa::path(post, path = "/planner/chains/{chain}/edges", tag = "planner", request_body = EdgeInput,
    params(("chain" = String, Path, description = CHAIN)),
    responses((status = 200, body = ChainDetail), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
#[deprecated(note = "POST /chains/{chain}/edges; kept for one release")]
/// Deprecated alias of `POST /chains/{chain}/edges`; the dependency is a plain one, as before.
pub async fn planner_add_dependency(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(chain): Path<String>,
    Json(e): Json<EdgeInput>,
) -> ApiResult<Json<ChainDetail>> {
    principal.planner_project()?;
    let e = EdgeInput {
        kind: EdgeKind::Normal,
        ..e
    };
    let detail = chains::add_dependency(&state, principal.caller(), chain, e).await?;
    Ok(Json(detail))
}

#[utoipa::path(delete, path = "/planner/chains/{chain}/edges", tag = "planner",
    params(("chain" = String, Path, description = CHAIN), EdgeInput),
    responses((status = 200, body = ChainDetail), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
#[deprecated(note = "DELETE /chains/{chain}/edges; kept for one release")]
/// Deprecated alias of `DELETE /chains/{chain}/edges`.
pub async fn planner_remove_dependency(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(chain): Path<String>,
    Query(e): Query<EdgeInput>,
) -> ApiResult<Json<ChainDetail>> {
    principal.planner_project()?;
    let detail = chains::remove_dependency(&state, principal.caller(), chain, e).await?;
    Ok(Json(detail))
}

#[utoipa::path(post, path = "/planner/chains/{chain}/cards/{card}/move", tag = "planner", request_body = PlannerMove,
    params(("chain" = String, Path, description = CHAIN), ("card" = String, Path, description = CARD)),
    responses((status = 200, body = ChainDetail), (status = 400, body = ErrorBody), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
#[deprecated(
    note = "POST /chains/{chain}/tickets/{ticket}/move or /tickets/{ticket}/move-to-chain; kept for one release"
)]
/// Deprecated alias of `POST /chains/{chain}/tickets/{ticket}/move` (next to a card) and
/// `POST /tickets/{ticket}/move-to-chain?from={chain}` (`to_chain`); answers the epic the card
/// is in afterwards.
pub async fn planner_move_card(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path((chain, card)): Path<(String, String)>,
    Json(m): Json<PlannerMove>,
) -> ApiResult<Json<ChainDetail>> {
    let project = principal.planner_project()?;
    let actor = principal.caller().actor();
    let detail = chain_write(&state, actor, move |tx| {
        let to = (m.to_chain.as_deref(), m.anchor.as_deref(), m.place);
        Ok(planner::move_card_by(tx, project, &chain, &card, to)?)
    })
    .await?;
    Ok(Json(detail))
}

#[utoipa::path(post, path = "/planner/cards/{card}/threads", tag = "planner", request_body = NewThread,
    params(("card" = String, Path, description = CARD)),
    responses((status = 201, body = ThreadDetail), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
#[deprecated(note = "POST /tickets/{ticket}/threads; kept for one release")]
/// Deprecated alias of `POST /tickets/{ticket}/threads`.
pub async fn planner_open_thread(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(card): Path<String>,
    Json(input): Json<NewThread>,
) -> ApiResult<(StatusCode, Json<ThreadDetail>)> {
    principal.planner_project()?;
    let detail = threads::open_as(&state, principal.caller(), card, input).await?;
    Ok((StatusCode::CREATED, detail))
}

#[utoipa::path(post, path = "/planner/cards/{card}/threads/{thread}/messages", tag = "planner", request_body = NewMessage,
    params(("card" = String, Path, description = CARD), ("thread" = String, Path, description = THREAD)),
    responses((status = 200, body = ThreadDetail), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
#[deprecated(note = "POST /tickets/{ticket}/threads/{thread}/messages; kept for one release")]
/// Deprecated alias of `POST /tickets/{ticket}/threads/{thread}/messages`.
pub async fn planner_thread_message(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(at): Path<(String, String)>,
    Json(input): Json<NewMessage>,
) -> ApiResult<Json<ThreadDetail>> {
    principal.planner_project()?;
    threads::message_as(&state, principal.caller(), at, input).await
}

#[utoipa::path(post, path = "/planner/cards/{card}/threads/{thread}/versions", tag = "planner", request_body = NewVersion,
    params(("card" = String, Path, description = CARD), ("thread" = String, Path, description = THREAD)),
    responses((status = 200, body = ThreadDetail), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
#[deprecated(note = "POST /tickets/{ticket}/threads/{thread}/versions; kept for one release")]
/// Deprecated alias of `POST /tickets/{ticket}/threads/{thread}/versions`.
pub async fn planner_thread_version(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(at): Path<(String, String)>,
    Json(input): Json<NewVersion>,
) -> ApiResult<Json<ThreadDetail>> {
    principal.planner_project()?;
    threads::version_as(&state, principal.caller(), at, input).await
}

#[utoipa::path(post, path = "/planner/suggestion", tag = "planner", request_body = NewSuggestion,
    responses((status = 200, body = Suggestion), (status = 400, body = ErrorBody), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
#[deprecated(note = "PUT /projects/{project}/suggestion; kept for one release")]
/// Deprecated alias of `PUT /projects/{project}/suggestion` for the planner's project.
pub async fn planner_suggest(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(new): Json<NewSuggestion>,
) -> ApiResult<Json<Suggestion>> {
    let project = principal.planner_project()?.to_string();
    Ok(Json(
        inbox::suggest_as(&state, principal.caller(), project, new).await?,
    ))
}
