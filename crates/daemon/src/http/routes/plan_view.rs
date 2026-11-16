//! The plan as any caller reads it, and a story shown in the window: `/projects/{project}/plan`,
//! `/plan/overview`, `/projects/{project}/designs/{design}`, `/chains/{chain}/plan`,
//! `/tickets/{ticket}/plan` and `POST /tickets/{ticket}/show`. Whoever calls, core's
//! `access::may` decides (a planner reads its own project only); the planner's old `/planner/...`
//! reads answer through these.

use crate::http::body::Json;
use axum::Extension;
use axum::extract::{Path, State};
use toknila_api_types::{PlanCard, PlanChain, PlannerDesign, PlannerOverview, PlannerPlan};
use toknila_app::access::{self, Action, Caller};
use toknila_app::{as_caller, planner, resolve};
use toknila_core::ProjectId;

use super::{ApiResult, load_ticket_detail, read, stamp};
use crate::designs::Designs;
use crate::http::dto::TicketDetail;
use crate::http::error::ErrorBody;
use crate::server::{AppState, Principal, now};

const PROJECT: &str = "Project id or key";

/// The project `project` names, once `caller` may read its plan.
async fn readable(state: &AppState, caller: Caller, project: String) -> ApiResult<ProjectId> {
    read(state, move |repo| {
        let p = resolve::project(repo, &project)?;
        access::on_project(repo, &caller, &Action::ReadPlan, p.id)?;
        Ok(p.id)
    })
    .await
}

/// The whole plan of `project` with its designs, as `caller` may read it.
pub(super) async fn plan_of(
    state: &AppState,
    caller: Caller,
    project: String,
) -> ApiResult<PlannerPlan> {
    let id = readable(state, caller, project).await?;
    let at = now();
    let plan = read(state, move |repo| Ok(planner::plan(repo, id, at)?)).await?;
    let designs = Designs::new(&state.home, id);
    let designs = tokio::task::spawn_blocking(move || designs.for_planner()).await??;
    Ok(PlannerPlan { plan, designs })
}

/// The plan of `project` in short records, as `caller` may read it.
pub(super) async fn overview_of(
    state: &AppState,
    caller: Caller,
    project: String,
) -> ApiResult<PlannerOverview> {
    let id = readable(state, caller, project).await?;
    let designs = Designs::new(&state.home, id);
    let designs = tokio::task::spawn_blocking(move || designs.for_planner()).await??;
    let names: Vec<_> = designs.iter().map(|d| (d.id, d.name.clone())).collect();
    let at = now();
    let overview = read(state, move |repo| {
        Ok(planner::overview(repo, id, at, &names)?)
    })
    .await?;
    Ok(PlannerOverview { overview, designs })
}

/// One design of `project`, with the path of its file, as `caller` may read it.
pub(super) async fn design_of(
    state: &AppState,
    caller: Caller,
    project: String,
    design: String,
) -> ApiResult<PlannerDesign> {
    let id = readable(state, caller, project).await?;
    let designs = Designs::new(&state.home, id);
    Ok(tokio::task::spawn_blocking(move || designs.find_for_planner(&design)).await??)
}

/// Asks the window to show a story, as `caller`; answers with the story.
pub(super) async fn show(
    state: &AppState,
    caller: Caller,
    story: String,
) -> ApiResult<TicketDetail> {
    let (at, prices) = stamp(state);
    let (detail, _) = state
        .store
        .write(caller.actor(), at, move |tx| -> ApiResult<TicketDetail> {
            let t = as_caller::show_story(tx, &caller, &story)?;
            load_ticket_detail(&tx.repo(), t.id, at, &prices)
        })
        .await?;
    Ok(detail)
}

#[utoipa::path(get, path = "/projects/{project}/plan", tag = "plan",
    params(("project" = String, Path, description = PROJECT)),
    responses((status = 200, body = PlannerPlan), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// The whole plan of a project: epics, stories in order, dependencies, threads, and the
/// project's designs with the path of each file. A planner reads its own project's.
pub async fn project_plan(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(project): Path<String>,
) -> ApiResult<Json<PlannerPlan>> {
    Ok(Json(plan_of(&state, principal.caller(), project).await?))
}

#[utoipa::path(get, path = "/projects/{project}/plan/overview", tag = "plan",
    params(("project" = String, Path, description = PROJECT)),
    responses((status = 200, body = PlannerOverview), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// The plan of a project in short records (every epic, the stories of the epics that are not
/// finished, the designs with their paths, the pins): small at any size of the plan.
pub async fn project_plan_overview(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(project): Path<String>,
) -> ApiResult<Json<PlannerOverview>> {
    Ok(Json(
        overview_of(&state, principal.caller(), project).await?,
    ))
}

#[utoipa::path(get, path = "/projects/{project}/designs/{design}", tag = "plan",
    params(("project" = String, Path, description = PROJECT),
        ("design" = String, Path, description = "Design id or name")),
    responses((status = 200, body = PlannerDesign), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// One design of a project: its metadata and the path of its file.
pub async fn get_design(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path((project, design)): Path<(String, String)>,
) -> ApiResult<Json<PlannerDesign>> {
    let found = design_of(&state, principal.caller(), project, design).await?;
    Ok(Json(found))
}

#[utoipa::path(get, path = "/chains/{chain}/plan", tag = "plan",
    params(("chain" = String, Path, description = "Chain id or name")),
    responses((status = 200, body = PlanChain), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// One epic with every story in full.
pub async fn chain_plan(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(chain): Path<String>,
) -> ApiResult<Json<PlanChain>> {
    let (caller, at) = (principal.caller(), now());
    let found = read(&state, move |repo| {
        Ok(planner::chain_in_full(repo, &caller, &chain, at)?)
    })
    .await?;
    Ok(Json(found))
}

#[utoipa::path(get, path = "/tickets/{ticket}/plan", tag = "plan",
    params(("ticket" = String, Path, description = "Ticket id, key or title")),
    responses((status = 200, body = PlanCard), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// One story in full: prompt, acceptance, subtasks, threads with their messages, review results
/// and pull request.
pub async fn ticket_plan(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(ticket): Path<String>,
) -> ApiResult<Json<PlanCard>> {
    let caller = principal.caller();
    let found = read(&state, move |repo| {
        Ok(planner::card_in_full(repo, &caller, &ticket)?)
    })
    .await?;
    Ok(Json(found))
}

#[utoipa::path(post, path = "/tickets/{ticket}/show", tag = "plan",
    params(("ticket" = String, Path, description = "Ticket id, key or title")),
    responses((status = 200, body = TicketDetail), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// Asks the window to show a story (a `window.show_card` event); nothing in the plan changes.
pub async fn show_ticket(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(ticket): Path<String>,
) -> ApiResult<Json<TicketDetail>> {
    Ok(Json(show(&state, principal.caller(), ticket).await?))
}
