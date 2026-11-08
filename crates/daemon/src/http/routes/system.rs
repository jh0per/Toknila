//! `/system/checks`, `/system/cli` and `/repos/check`: what the window needs to set a project up
//! and to put `tok` on the person's command line.

use axum::Json;
use axum::extract::State;
use toknila_api_types::{CliLink, Places};
use toknila_core::Project;
use toknila_core::repo::{NoDefaultBase, RepoCheck, RepoProblem};
use toknila_core::system::SystemChecks;

use super::{ApiResult, read};
use crate::http::dto::RepoCheckRequest;
use crate::server::AppState;
use crate::tools;

#[utoipa::path(get, path = "/system/checks", tag = "system",
    responses((status = 200, body = SystemChecks)))]
/// Is `git` new enough, is `claude` installed, is `gh` logged in; each with a fix hint.
pub async fn system_checks(State(state): State<AppState>) -> ApiResult<Json<SystemChecks>> {
    let t = state.tools.clone();
    let commands = read(&state, |repo| {
        Ok(repo
            .projects()?
            .into_iter()
            .filter(|p| !p.archived)
            .filter_map(|p| p.check_command)
            .collect::<Vec<_>>())
    })
    .await?;
    let checks =
        tokio::task::spawn_blocking(move || tools::system_checks_for(t.as_ref(), &commands))
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

/// The base branch a repository suggests, on a blocking thread (it asks the repository).
pub async fn default_base(
    state: &AppState,
    path: String,
) -> ApiResult<Result<String, NoDefaultBase>> {
    let t = state.tools.clone();
    tokio::task::spawn_blocking(move || tools::default_base(t.as_ref(), &path))
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
    // No base named: the repository's own default, so the form can show it as the value.
    let base = match input.work_branch.filter(|b| !b.trim().is_empty()) {
        Some(b) => b,
        None => default_base(&state, input.repo_path.clone())
            .await?
            .unwrap_or_else(|_| Project::DEFAULT_WORK_BRANCH.to_owned()),
    };
    Ok(Json(check_repo(&state, input.repo_path, base).await?))
}

/// The daemon's own binary, the person's home folder and `PATH`, as the link needs them.
fn environment() -> (
    std::path::PathBuf,
    Option<std::path::PathBuf>,
    Option<std::ffi::OsString>,
) {
    (
        std::env::current_exe().unwrap_or_default(),
        dirs::home_dir(),
        std::env::var_os("PATH"),
    )
}

#[utoipa::path(get, path = "/system/places", tag = "system",
    responses((status = 200, body = Places)))]
/// Where the data home, the ships and the profiles folder are on this machine.
pub async fn system_places(State(state): State<AppState>) -> Json<Places> {
    let text = |p: &std::path::Path| p.display().to_string();
    Json(Places {
        home: text(&state.home),
        ships: text(&state.ships),
        profiles: text(&state.profiles),
    })
}

#[utoipa::path(get, path = "/system/cli", tag = "system",
    responses((status = 200, body = CliLink)))]
/// The Toknila version, the `tok` that ships next to the daemon and whether `~/.local/bin/tok`
/// links to it.
pub async fn cli_status() -> Json<CliLink> {
    let (exe, home, path) = environment();
    Json(crate::cli_link::status(
        &exe,
        home.as_deref(),
        path.as_deref(),
    ))
}

#[utoipa::path(post, path = "/system/cli/install", tag = "system",
    responses((status = 200, body = CliLink)))]
/// Links `~/.local/bin/tok` to the bundled `tok`. Only the person's home is written, never with
/// elevated rights.
pub async fn cli_install() -> ApiResult<Json<CliLink>> {
    let (exe, home, path) = environment();
    crate::cli_link::install(&exe, home.as_deref(), path.as_deref())
        .map(Json)
        .map_err(|e| toknila_app::AppError::conflict(e).into())
}
