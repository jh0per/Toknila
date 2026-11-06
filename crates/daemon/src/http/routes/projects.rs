//! `/projects`.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use toknila_app::{projects, resolve, reviews};
use toknila_core::{Actor, Project};

use super::{ApiResult, read};
use crate::http::dto::{NewProject, ReviewSettings};
use crate::http::error::ErrorBody;
use crate::server::{AppState, now};

#[utoipa::path(get, path = "/projects", tag = "projects",
    responses((status = 200, body = Vec<Project>)))]
pub async fn list_projects(State(state): State<AppState>) -> ApiResult<Json<Vec<Project>>> {
    Ok(Json(read(&state, |repo| Ok(repo.projects()?)).await?))
}

#[utoipa::path(post, path = "/projects", tag = "projects", request_body = NewProject,
    responses((status = 201, body = Project), (status = 400, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn create_project(
    State(state): State<AppState>,
    Json(input): Json<NewProject>,
) -> ApiResult<(StatusCode, Json<Project>)> {
    let (p, _) = state
        .store
        .write(Actor::Human, now(), move |tx| -> ApiResult<Project> {
            Ok(projects::create(tx, input)?)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(p)))
}

#[utoipa::path(patch, path = "/projects/{project}", tag = "projects", request_body = ReviewSettings,
    params(("project" = String, Path, description = "Project id or key")),
    responses((status = 200, body = Project), (status = 404, body = ErrorBody)))]
/// Sets how the project's tickets are reviewed by default: the mode and the reviewer agent
/// profile. A chain or ticket that sets its own keeps it; a field left out is kept.
pub async fn patch_project(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Json(settings): Json<ReviewSettings>,
) -> ApiResult<Json<Project>> {
    let (p, _) = state
        .store
        .write(Actor::Human, now(), move |tx| -> ApiResult<Project> {
            Ok(reviews::set_project(tx, &project, settings)?)
        })
        .await?;
    Ok(Json(p))
}

#[utoipa::path(get, path = "/projects/{project}", tag = "projects",
    params(("project" = String, Path, description = "Project id or key")),
    responses((status = 200, body = Project), (status = 404, body = ErrorBody)))]
pub async fn get_project(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> ApiResult<Json<Project>> {
    Ok(Json(
        read(&state, move |repo| Ok(resolve::project(repo, &project)?)).await?,
    ))
}
