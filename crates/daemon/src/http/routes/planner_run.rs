//! `/projects/{project}/planner/...`: a person starts and stops the project's planner and uses its
//! terminal. The live output is the WebSocket `/projects/{project}/planner/stream`
//! (`crate::runner::planner_stream`). Every route here runs as a person; the planner's own routes
//! are `/planner/...` (`planner.rs`).

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use toknila_api_types::runner::{HEADER_END, HEADER_OFFSET, HEADER_RUN};
use toknila_api_types::{PlannerState, StartPlanner, TerminalSize};
use toknila_app::planner;
use toknila_app::resolve;
use toknila_core::{Actor, AgentProfile, AgentRole, PlannerSpec, Project, planner as rules};

use super::ApiResult;
use crate::http::error::{ApiError, ErrorBody};
use crate::server::AppState;

const PROJECT: &str = "Project id or key";

fn header(name: &'static str, value: impl ToString) -> (HeaderName, HeaderValue) {
    let value = HeaderValue::from_str(&value.to_string()).unwrap_or(HeaderValue::from_static(""));
    (HeaderName::from_static(name), value)
}

async fn project_of(state: &AppState, reference: String) -> ApiResult<Project> {
    Ok(state
        .store
        .read(move |repo| Ok(resolve::project(repo, &reference)))
        .await??)
}

fn state_of(state: &AppState, project: &Project) -> PlannerState {
    let live = state
        .runners
        .live_run(rules::run_key(project.id), AgentRole::Planner);
    PlannerState {
        running: live.is_some(),
        profile: live.as_ref().map_or_else(
            || AgentProfile::CLAUDE_CODE_PLANNER.to_owned(),
            |r| r.profile.clone(),
        ),
        run: live.map(|r| r.id),
    }
}

#[utoipa::path(get, path = "/projects/{project}/planner", tag = "planner",
    params(("project" = String, Path, description = PROJECT)),
    responses((status = 200, body = PlannerState), (status = 404, body = ErrorBody)))]
/// Whether the project's planner is running.
pub async fn get_planner(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> ApiResult<Json<PlannerState>> {
    let p = project_of(&state, project).await?;
    Ok(Json(state_of(&state, &p)))
}

#[utoipa::path(post, path = "/projects/{project}/planner/start", tag = "planner", request_body = StartPlanner,
    params(("project" = String, Path, description = PROJECT)),
    responses((status = 200, body = PlannerState), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
/// Starts the project's planner in its repository (Claude Code, read-only, with the planner
/// tools), or keeps the live one.
pub async fn start_planner(
    State(state): State<AppState>,
    Path(project): Path<String>,
    body: Option<Json<StartPlanner>>,
) -> ApiResult<Json<PlannerState>> {
    let p = project_of(&state, project).await?;
    let name = body
        .and_then(|Json(b)| b.profile)
        .unwrap_or_else(|| AgentProfile::CLAUDE_CODE_PLANNER.to_owned());
    let profile = crate::profiles::load(&state.profiles)
        .profiles
        .into_iter()
        .map(|l| l.profile)
        .find(|prof| prof.name == name)
        .ok_or_else(|| ApiError::bad_request(format!("agent profile {name} does not exist")))?;
    let runners = state.runners.clone();
    // A live planner is kept as it is: its checkout is not moved under it.
    let yard = if state_of(&state, &p).running {
        None
    } else {
        state.store.shipyard()
    };
    let started = {
        let p = p.clone();
        tokio::task::spawn_blocking(move || {
            // Fetches (bounded) and prepares the place the planner reads the code from.
            let site = yard
                .and_then(|yard| PlannerSpec::for_project(&p).map(|spec| yard.planner_site(&spec)));
            planner::start(&*runners, Actor::Human, &p, &profile, site)
        })
        .await
        .map_err(|e| ApiError(toknila_app::AppError::internal(e.to_string())))??
    };
    let mut answer = state_of(&state, &p);
    answer.run = Some(started.run.id);
    Ok(Json(answer))
}

#[utoipa::path(post, path = "/projects/{project}/planner/stop", tag = "planner",
    params(("project" = String, Path, description = PROJECT)),
    responses((status = 200, body = PlannerState), (status = 404, body = ErrorBody)))]
/// Stops the project's planner; one that is not running is fine.
pub async fn stop_planner(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> ApiResult<Json<PlannerState>> {
    let p = project_of(&state, project).await?;
    // Ending the process tree waits for a grace period; not on an async worker.
    let (runners, target) = (state.runners.clone(), p.clone());
    tokio::task::spawn_blocking(move || planner::stop(&*runners, Actor::Human, &target))
        .await
        .map_err(|e| ApiError(toknila_app::AppError::internal(e.to_string())))??;
    Ok(Json(state_of(&state, &p)))
}

#[utoipa::path(get, path = "/projects/{project}/planner/scrollback", tag = "planner",
    params(("project" = String, Path, description = PROJECT)),
    responses(
        (status = 200, description = "Raw terminal output of the planner's newest run, last lines only (ANSI kept). Headers x-toknila-run, x-toknila-offset (of the first byte) and x-toknila-end (after the last; continue the stream from here).", body = Vec<u8>, content_type = "application/octet-stream"),
        (status = 404, body = ErrorBody)))]
pub async fn planner_scrollback(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> ApiResult<Response> {
    let p = project_of(&state, project).await?;
    let runners = state.runners.clone();
    let key = rules::run_key(p.id);
    let chunk = tokio::task::spawn_blocking(move || runners.read(key, AgentRole::Planner, None))
        .await
        .map_err(|e| ApiError(toknila_app::AppError::internal(e.to_string())))?
        .ok_or_else(|| ApiError::not_found(format!("output of the planner of {}", p.key)))?;
    let headers = HeaderMap::from_iter([
        header(HEADER_RUN, chunk.run),
        header(HEADER_OFFSET, chunk.offset),
        header(HEADER_END, chunk.end),
    ]);
    Ok((headers, chunk.bytes).into_response())
}

#[utoipa::path(post, path = "/projects/{project}/planner/input", tag = "planner",
    params(("project" = String, Path, description = PROJECT)),
    request_body(content = Vec<u8>, content_type = "application/octet-stream", description = "Bytes typed into the planner's terminal, as they are"),
    responses((status = 204, description = "Written"), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn planner_input(
    State(state): State<AppState>,
    Path(project): Path<String>,
    body: Bytes,
) -> ApiResult<StatusCode> {
    let p = project_of(&state, project).await?;
    planner::send_input(&*state.runners, Actor::Human, &p, &body)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/projects/{project}/planner/size", tag = "planner",
    params(("project" = String, Path, description = PROJECT)),
    request_body = TerminalSize,
    responses((status = 204, description = "Resized; sizes outside 20x5..500x200 are brought into that range"), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
/// The size of the panel that shows the planner's terminal.
pub async fn planner_size(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Json(size): Json<TerminalSize>,
) -> ApiResult<StatusCode> {
    let p = project_of(&state, project).await?;
    planner::resize(&*state.runners, Actor::Human, &p, size.cols, size.rows)?;
    Ok(StatusCode::NO_CONTENT)
}
