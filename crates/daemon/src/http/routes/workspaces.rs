//! `/projects/{project}/ships`: the workspaces of a project's stories, and retiring those of
//! finished stories.

use axum::Json;
use axum::extract::{Path, State};
use toknila_app::{finished_ships, resolve};
use toknila_core::{Actor, slugify};

use super::{ApiResult, read};
use crate::http::dto::{Workspace, WorkspaceKind, Workspaces, WorkspacesRetired};
use crate::http::error::ErrorBody;
use crate::server::{AppState, now};

#[utoipa::path(get, path = "/projects/{project}/ships", tag = "projects",
    params(("project" = String, Path, description = "Project id or key")),
    responses((status = 200, body = Workspaces), (status = 404, body = ErrorBody)))]
/// Lists the workspaces of the project's stories with each story's status, and the assistant's
/// reading copy (`_planner`) named as such. The copy is not counted; `finished` counts the
/// workspaces of merged, released and cancelled stories.
pub async fn list_workspaces(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> ApiResult<Json<Workspaces>> {
    let (key, rows) = read(&state, move |repo| {
        let p = resolve::project(repo, &project)?;
        Ok((
            p.key.to_string(),
            finished_ships::workspaces(repo, Some(p.id))?,
        ))
    })
    .await?;
    let mut items = Vec::new();
    let planner = state.ships.join(slugify(&key)).join("_planner");
    if planner.is_dir() {
        items.push(Workspace {
            kind: WorkspaceKind::Planner,
            story: None,
            status: None,
            branch: None,
            path: planner.to_string_lossy().into_owned(),
            finished: false,
        });
    }
    let count = rows.len() as u32;
    let finished = rows.iter().filter(|w| w.finished()).count() as u32;
    items.extend(rows.into_iter().map(|w| Workspace {
        kind: WorkspaceKind::Story,
        story: Some(w.ticket.key.to_string()),
        status: Some(w.ticket.status),
        branch: Some(w.ship.branch.clone()),
        finished: w.finished(),
        path: w.ship.path,
    }));
    Ok(Json(Workspaces {
        items,
        count,
        finished,
    }))
}

#[utoipa::path(post, path = "/projects/{project}/ships/retire-finished", tag = "projects",
    params(("project" = String, Path, description = "Project id or key")),
    responses((status = 200, body = WorkspacesRetired), (status = 404, body = ErrorBody)))]
/// Retires the workspace of every merged, released or cancelled story of the project: the
/// worktree goes, the branch goes unless it holds work the base lacks (the story says so), and
/// the record goes. Only the person's token may call it.
pub async fn retire_finished_workspaces(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> ApiResult<Json<WorkspacesRetired>> {
    let (queued, _) = state
        .store
        .write(Actor::Human, now(), move |tx| -> ApiResult<_> {
            let p = resolve::project(tx, &project)?;
            Ok(finished_ships::retire_finished(tx, Some(p.id))?)
        })
        .await?;
    Ok(Json(WorkspacesRetired {
        retired: finished_ships::keys(&queued)
            .iter()
            .map(ToString::to_string)
            .collect(),
    }))
}
