//! `POST /chains/{chain}/release/mark`: an epic shipped inside a tag that already exists.

use axum::extract::{Path, State};
use toknila_app::releases;
use toknila_app::{AppError, resolve};
use toknila_core::Actor;

use super::releases::repo_of;
use super::{ApiResult, chain_write, read};
use crate::http::body::Json;
use crate::http::dto::{ChainDetail, MarkRelease};
use crate::http::error::ErrorBody;
use crate::release_carry;
use crate::server::{AppState, now};

#[utoipa::path(post, path = "/chains/{chain}/release/mark", tag = "releases", request_body = MarkRelease,
    params(("chain" = String, Path, description = "Chain id or name")),
    responses((status = 200, body = ChainDetail), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
/// A person says the epic shipped inside an existing tag: its release node reads released with
/// that tag. Refused with a sentence when the tag does not exist or does not contain the merges of
/// every story of the epic.
pub async fn mark_release(
    State(state): State<AppState>,
    Path(chain): Path<String>,
    Json(input): Json<MarkRelease>,
) -> ApiResult<Json<ChainDetail>> {
    let tag = input.tag.trim().to_owned();
    let asked = tag.clone();
    let (epic, project, project_id) = read(&state, move |repo| {
        let c = resolve::chain(repo, &chain)?;
        let view = repo
            .chain_view(c.id)?
            .ok_or_else(|| AppError::not_found("epic"))?;
        // The same refusals the marking gives, before git is asked anything.
        view.clone().release_with(&asked, None, now())?;
        let project = repo
            .project(c.project_id)?
            .ok_or_else(|| AppError::not_found("project"))?;
        let epic = releases::FinishedEpic::of(&view)
            .ok_or_else(|| AppError::conflict("the epic is not finished"))?;
        Ok((epic, project, c.project_id))
    })
    .await?;
    let repo = repo_of(&project)?;
    let found = tokio::task::spawn_blocking(move || {
        let (name, commit) = release_carry::tag_commit(&repo, &tag)?;
        let inside = release_carry::contained(&repo, &commit, std::slice::from_ref(&epic))?;
        if inside.is_empty() {
            return Err(format!(
                "the tag {name} does not contain the merges of epic {}",
                epic.name
            ));
        }
        Ok((name, epic.chain))
    })
    .await
    .map_err(|e| AppError::internal(e.to_string()))?
    .map_err(AppError::conflict)?;
    let (tag, chain_id) = found;
    let detail = chain_write(&state, Actor::Human, move |tx| {
        let by = releases::cutter_of(&*tx, project_id, &tag)?;
        releases::mark_with(tx, chain_id, &tag, by.as_deref())?;
        Ok(chain_id)
    })
    .await?;
    Ok(Json(detail))
}
