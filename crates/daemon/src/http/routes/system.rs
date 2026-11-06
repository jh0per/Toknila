//! `/system/checks` and `/repos/check`: what the window needs to set a project up.

use axum::Json;
use axum::extract::State;
use toknila_core::Project;
use toknila_core::repo::{RepoCheck, RepoProblem};
use toknila_core::system::SystemChecks;

use super::ApiResult;
use crate::http::dto::RepoCheckRequest;
use crate::server::AppState;
use crate::tools;

#[utoipa::path(get, path = "/system/checks", tag = "system",
    responses((status = 200, body = SystemChecks)))]
/// Is `git` new enough, is `claude` installed, is `gh` logged in; each with a fix hint.
pub async fn system_checks(State(state): State<AppState>) -> ApiResult<Json<SystemChecks>> {
    let t = state.tools.clone();
    let checks = tokio::task::spawn_blocking(move || tools::system_checks(t.as_ref()))
        .await
        .map_err(|e| toknila_app::AppError::internal(e.to_string()))?;
    Ok(Json(checks))
}

/// The folder check on a blocking thread (it runs `git`).
pub async fn check_repo(state: &AppState, path: String, base: String) -> ApiResult<RepoCheck> {
    let t = state.tools.clone();
    tokio::task::spawn_blocking(move || tools::check_repo(t.as_ref(), &path, &base))
        .await
        .map_err(|e| toknila_app::AppError::internal(e.to_string()).into())
}

/// The first problem of a check, as a refusal.
pub fn first_problem(check: RepoCheck) -> Result<(), RepoProblem> {
    check.problems.into_iter().next().map_or(Ok(()), Err)
}

#[utoipa::path(post, path = "/repos/check", tag = "system", request_body = RepoCheckRequest,
    responses((status = 200, body = RepoCheck)))]
/// Is this folder a git repository with the base branch? The branches come back so a form can
/// offer them when the base is missing.
pub async fn repos_check(
    State(state): State<AppState>,
    Json(input): Json<RepoCheckRequest>,
) -> ApiResult<Json<RepoCheck>> {
    let base = input
        .work_branch
        .unwrap_or_else(|| Project::DEFAULT_WORK_BRANCH.to_owned());
    Ok(Json(check_repo(&state, input.repo_path, base).await?))
}
