//! `/projects`.

use crate::http::body::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use toknila_app::removal::{self, Removed};
use toknila_app::{projects, resolve};
use toknila_core::repo::RepoField;
use toknila_core::{Actor, Project};

use super::system::{check_repo, default_base, first_problem};
use super::{ApiResult, read};
use crate::http::dto::{DeleteProject, NewProject, PatchProject, ProjectDeletion};
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
    Json(mut input): Json<NewProject>,
) -> Result<(StatusCode, Json<Project>), FormError> {
    if let Some(Err(e)) = input
        .check_command
        .as_deref()
        .map(Project::clean_check_command)
    {
        return Err(FormError::field(
            "check_command",
            "check_command_invalid",
            e,
        ));
    }
    if let Some(Err(e)) = input
        .setup_command
        .as_deref()
        .map(Project::clean_setup_command)
    {
        return Err(FormError::field(
            "setup_command",
            "setup_command_invalid",
            e,
        ));
    }
    if let Some(path) = input.repo_path.clone() {
        // No base named: the branch the repository is on, else its remote's default.
        if input
            .work_branch
            .as_deref()
            .is_none_or(|b| b.trim().is_empty())
        {
            let found = default_base(&state, path.clone()).await?;
            // A folder that is no repository is that field's problem, not the branch's.
            let tried = found
                .clone()
                .unwrap_or_else(|_| Project::DEFAULT_WORK_BRANCH.to_owned());
            let check = check_repo(&state, path.clone(), tried).await?;
            if let Some(p) = check
                .problems
                .into_iter()
                .find(|p| p.field == RepoField::RepoPath)
            {
                return Err(p.into());
            }
            input.work_branch =
                Some(found.map_err(|e| {
                    FormError::field("work_branch", "branch_missing", e.to_string())
                })?);
        }
        let base = input.work_branch.clone().unwrap_or_default();
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

async fn set_archived(
    state: &AppState,
    project: String,
    archived: bool,
) -> ApiResult<Json<Project>> {
    let (p, _) = state
        .store
        .write(Actor::Human, now(), move |tx| -> ApiResult<Project> {
            Ok(projects::set_archived(tx, &project, archived)?)
        })
        .await?;
    Ok(Json(p))
}

#[utoipa::path(post, path = "/projects/{project}/archive", tag = "projects",
    params(("project" = String, Path, description = "Project id or key")),
    responses((status = 200, body = Project), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
/// Hides the project from the sidebar and keeps its data. Refused while its agents work.
pub async fn archive_project(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> ApiResult<Json<Project>> {
    set_archived(&state, project, true).await
}

#[utoipa::path(post, path = "/projects/{project}/unarchive", tag = "projects",
    params(("project" = String, Path, description = "Project id or key")),
    responses((status = 200, body = Project), (status = 404, body = ErrorBody)))]
/// Brings an archived project back to the sidebar.
pub async fn unarchive_project(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> ApiResult<Json<Project>> {
    set_archived(&state, project, false).await
}

#[utoipa::path(post, path = "/projects/{project}/delete", tag = "projects", request_body = DeleteProject,
    params(("project" = String, Path, description = "Project id or key")),
    responses((status = 200, body = ProjectDeletion), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
/// Deletes the project: its chains, cards, milestones and everything attached to them, with
/// their journal entries, in one transaction; refused (409) while any of its agents works. Then
/// the ships' worktrees are removed from the repository and the cards' run files deleted; with
/// `trash_folder` the repository folder goes to the system Trash. Nothing else in the repository
/// is touched: its `ship/*` branches stay. A cleanup step that fails is listed in `warnings` (or
/// `folder_error`); the deletion stands either way.
pub async fn delete_project(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Json(input): Json<DeleteProject>,
) -> ApiResult<Json<ProjectDeletion>> {
    let (removed, _) = state
        .store
        .write(Actor::Human, now(), move |tx| -> ApiResult<Removed> {
            Ok(removal::delete_project(tx, &project)?)
        })
        .await?;
    Ok(Json(
        crate::cleanup::after_delete(&state, removed, input.trash_folder).await,
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
    if let Some(Err(e)) = patch.max_review_rounds.map(Project::check_review_rounds) {
        return Err(FormError::field(
            "max_review_rounds",
            "rounds_out_of_range",
            e,
        ));
    }
    if let Some(Err(e)) = patch
        .check_command
        .as_deref()
        .map(Project::clean_check_command)
    {
        return Err(FormError::field(
            "check_command",
            "check_command_invalid",
            e,
        ));
    }
    if let Some(Err(e)) = patch
        .setup_command
        .as_deref()
        .map(Project::clean_setup_command)
    {
        return Err(FormError::field(
            "setup_command",
            "setup_command_invalid",
            e,
        ));
    }
    for (field, text) in [
        ("worker_model", &patch.worker_model),
        ("reviewer_model", &patch.reviewer_model),
        ("planner_model", &patch.planner_model),
    ] {
        if let Some(Err(e)) = text.as_deref().map(Project::clean_model) {
            return Err(FormError::field(field, "model_invalid", e));
        }
    }
    if let Some(Err(e)) = patch
        .allowed_commands
        .clone()
        .map(toknila_core::command_policy::clean_rules)
    {
        return Err(FormError::field("allowed_commands", "rule_invalid", e));
    }
    if let Some(Err(e)) = patch
        .reviewer_web_domains
        .clone()
        .map(toknila_core::command_policy::clean_domains)
    {
        return Err(FormError::field(
            "reviewer_web_domains",
            "domain_invalid",
            e,
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
