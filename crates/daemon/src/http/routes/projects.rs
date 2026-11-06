//! `/projects`.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use toknila_app::{projects, resolve};
use toknila_core::{Actor, Project};

use super::system::{check_repo, first_problem};
use super::{ApiResult, read};
use crate::http::dto::{NewProject, PatchProject};
use crate::http::error::{ErrorBody, FormError};
use crate::server::{AppState, now};

#[utoipa::path(get, path = "/projects", tag = "projects",
    responses((status = 200, body = Vec<Project>)))]
pub async fn list_projects(State(state): State<AppState>) -> ApiResult<Json<Vec<Project>>> {
    Ok(Json(read(&state, |repo| Ok(repo.projects()?)).await?))
}

#[utoipa::path(post, path = "/projects", tag = "projects", request_body = NewProject,
    responses((status = 201, body = Project), (status = 400, body = ErrorBody), (status = 409, body = ErrorBody)))]
/// Creates a project. A folder (`repo_path`) must be a repository with the base branch; the
/// refusal names the field.
pub async fn create_project(
    State(state): State<AppState>,
    Json(input): Json<NewProject>,
) -> Result<(StatusCode, Json<Project>), FormError> {
    if let Some(path) = input.repo_path.clone() {
        let base = input
            .work_branch
            .clone()
            .unwrap_or_else(|| Project::DEFAULT_WORK_BRANCH.to_owned());
        first_problem(check_repo(&state, path, base).await?)?;
    }
    let (p, _) = state
        .store
        .write(Actor::Human, now(), move |tx| -> ApiResult<Project> {
            Ok(projects::create(tx, input)?)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(p)))
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

#[utoipa::path(patch, path = "/projects/{project}", tag = "projects", request_body = PatchProject,
    params(("project" = String, Path, description = "Project id or key")),
    responses((status = 200, body = Project), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// Changes project settings. A higher parallel limit starts waiting cards at once; a lower one
/// never stops a running agent. A new base branch must exist in the project's repository and a
/// default profile must be in the catalog; the refusal names the field.
pub async fn patch_project(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Json(patch): Json<PatchProject>,
) -> Result<Json<Project>, FormError> {
    if let Some(Err(e)) = patch.max_parallel.map(toknila_core::schedule::check_limit) {
        return Err(FormError::field(
            "max_parallel",
            "limit_out_of_range",
            e.to_string(),
        ));
    }
    if let Some(profile) = &patch.default_profile {
        let catalog = crate::profiles::load(&state.profiles);
        if !catalog
            .profiles
            .iter()
            .any(|l| l.profile.name == profile.trim())
        {
            return Err(FormError::field(
                "default_profile",
                "profile_unknown",
                format!("There is no profile {profile:?}."),
            ));
        }
    }
    if let Some(base) = patch.work_branch.clone() {
        let key = project.clone();
        let p = read(&state, move |repo| Ok(resolve::project(repo, &key)?)).await?;
        if let Some(path) = p.repo_path {
            first_problem(check_repo(&state, path, base).await?)?;
        }
    }
    let (p, _) = state
        .store
        .write(Actor::Human, now(), move |tx| -> ApiResult<Project> {
            Ok(projects::patch(tx, &project, patch)?)
        })
        .await?;
    Ok(Json(p))
}
