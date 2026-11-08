//! `/projects/{project}/strays`: what cancelled or deleted cards left in the repository.

use axum::Json;
use axum::extract::{Path, State};
use toknila_app::{AppError, resolve, strays};
use toknila_core::Project;
use toknila_core::strays::{Owned, Stray, StrayKind, find};

use super::{ApiResult, read};
use crate::http::dto::{Strays, StraysCleaned};
use crate::http::error::ErrorBody;
use crate::plan_files::canonical;
use crate::server::AppState;
use crate::store::Yard;

/// The project, and what its live cards hold.
async fn context(state: &AppState, project: String) -> ApiResult<(Project, Owned)> {
    read(state, move |repo| {
        let p = resolve::project(repo, &project)?;
        let owned = strays::owned(repo, &p)?;
        Ok((p, owned))
    })
    .await
}

/// Looks at the repository on a blocking thread and keeps what no live card owns.
fn look(yard: &Yard, project: &Project, owned: &Owned) -> Result<Vec<Stray>, AppError> {
    let Some(repo) = project
        .repo_path
        .as_deref()
        .filter(|r| !r.trim().is_empty())
    else {
        return Ok(Vec::new());
    };
    let owned = Owned {
        worktrees: owned
            .worktrees
            .iter()
            .map(|p| {
                canonical(std::path::Path::new(p))
                    .to_string_lossy()
                    .into_owned()
            })
            .collect(),
        branches: owned.branches.clone(),
    };
    let found = yard
        .found(repo, &project.work_branch)
        .map_err(|e| AppError::conflict(format!("cannot look at the repository: {e}")))?;
    Ok(find(&found, &owned))
}

#[utoipa::path(get, path = "/projects/{project}/strays", tag = "projects",
    params(("project" = String, Path, description = "Project id or key")),
    responses((status = 200, body = Strays), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
/// Lists the ships' worktrees and `ship/*` branches in the project's repository that no live
/// card owns: what cancelled or deleted cards left behind. Nothing is changed.
pub async fn list_strays(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> ApiResult<Json<Strays>> {
    let (p, owned) = context(&state, project).await?;
    let Some(yard) = state.store.shipyard() else {
        return Ok(Json(Strays { items: Vec::new() }));
    };
    let items = tokio::task::spawn_blocking(move || look(&yard, &p, &owned))
        .await
        .map_err(|e| AppError::new(toknila_app::ErrorKind::Internal, e.to_string()))??;
    Ok(Json(Strays { items }))
}

#[utoipa::path(post, path = "/projects/{project}/strays/clean", tag = "projects",
    params(("project" = String, Path, description = "Project id or key")),
    responses((status = 200, body = StraysCleaned), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
/// Removes what `GET .../strays` lists, looked at again now: stray worktrees first (their
/// checkouts are discarded), then stray branches, whatever commits they hold. A step that fails
/// is named in `warnings` and that leftover stays.
pub async fn clean_strays(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> ApiResult<Json<StraysCleaned>> {
    let (p, owned) = context(&state, project).await?;
    let Some(yard) = state.store.shipyard() else {
        return Ok(Json(StraysCleaned {
            removed: Vec::new(),
            warnings: Vec::new(),
        }));
    };
    let done = tokio::task::spawn_blocking(move || -> Result<StraysCleaned, AppError> {
        let mut items = look(&yard, &p, &owned)?;
        items.sort_by_key(|s| s.kind == StrayKind::Branch);
        let repo = p.repo_path.clone().unwrap_or_default();
        let mut out = StraysCleaned {
            removed: Vec::new(),
            warnings: Vec::new(),
        };
        for s in items {
            match yard.discard(&repo, &s) {
                Ok(()) => out.removed.push(s),
                Err(e) => out.warnings.push(format!("{} stays: {e}", s.name)),
            }
        }
        Ok(out)
    })
    .await
    .map_err(|e| AppError::new(toknila_app::ErrorKind::Internal, e.to_string()))??;
    Ok(Json(done))
}
