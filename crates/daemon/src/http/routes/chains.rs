//! `/projects/{project}/chains`, `/chains`.

use std::collections::HashMap;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use toknila_app::{chains, resolve, reviews};
use toknila_core::Actor;

use super::{ApiResult, chain_write, load_chain_detail, read, stamp};
use crate::http::dto::{
    ChainDetail, ChainFile, ChainItem, EdgeInput, MoveInput, NewChain, ReviewSettings,
};
use crate::http::error::ErrorBody;
use crate::server::{AppState, now};

#[utoipa::path(post, path = "/projects/{project}/chains", tag = "chains", request_body = NewChain,
    params(("project" = String, Path, description = "Project id or key")),
    responses((status = 201, body = ChainDetail), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn create_chain(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Json(input): Json<NewChain>,
) -> ApiResult<(StatusCode, Json<ChainDetail>)> {
    let detail = chain_write(&state, Actor::Human, move |tx| {
        Ok(chains::create(tx, &project, input)?)
    })
    .await?;
    Ok((StatusCode::CREATED, Json(detail)))
}

#[utoipa::path(post, path = "/projects/{project}/import", tag = "chains", request_body = ChainFile,
    params(("project" = String, Path, description = "Project id or key")),
    responses((status = 201, body = ChainDetail), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn import_chain(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Json(input): Json<ChainFile>,
) -> ApiResult<(StatusCode, Json<ChainDetail>)> {
    let detail = chain_write(&state, Actor::Human, move |tx| {
        Ok(chains::import(tx, &project, &input.toml)?)
    })
    .await?;
    Ok((StatusCode::CREATED, Json(detail)))
}

#[utoipa::path(get, path = "/chains/{chain}/export", tag = "chains",
    params(("chain" = String, Path, description = "Chain id or name")),
    responses((status = 200, body = ChainFile), (status = 404, body = ErrorBody)))]
pub async fn export_chain(
    State(state): State<AppState>,
    Path(chain): Path<String>,
) -> ApiResult<Json<ChainFile>> {
    let toml = read(&state, move |repo| Ok(chains::export(repo, &chain)?)).await?;
    Ok(Json(ChainFile { toml }))
}

#[utoipa::path(get, path = "/chains", tag = "chains",
    params(("project" = Option<String>, Query, description = "Project id or key")),
    responses((status = 200, body = Vec<ChainItem>)))]
pub async fn list_chains(
    State(state): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Json<Vec<ChainItem>>> {
    let at = now();
    let items = read(&state, move |repo| {
        let project = q
            .get("project")
            .map(|p| resolve::project(repo, p))
            .transpose()?;
        let mut out = Vec::new();
        for c in repo.chains(project.map(|p| p.id))? {
            if let Some(v) = repo.chain_view(c.id)? {
                out.push(ChainItem {
                    summary: v.summary(at),
                    chain: c,
                });
            }
        }
        Ok(out)
    })
    .await?;
    Ok(Json(items))
}

#[utoipa::path(get, path = "/chains/{chain}", tag = "chains",
    params(("chain" = String, Path, description = "Chain id or name")),
    responses((status = 200, body = ChainDetail), (status = 404, body = ErrorBody)))]
pub async fn get_chain(
    State(state): State<AppState>,
    Path(chain): Path<String>,
) -> ApiResult<Json<ChainDetail>> {
    let (at, prices) = stamp(&state);
    Ok(Json(
        read(&state, move |repo| {
            let c = resolve::chain(repo, &chain)?;
            load_chain_detail(repo, c.id, at, &prices)
        })
        .await?,
    ))
}

#[utoipa::path(patch, path = "/chains/{chain}", tag = "chains", request_body = ReviewSettings,
    params(("chain" = String, Path, description = "Chain id or name")),
    responses((status = 200, body = ChainDetail), (status = 404, body = ErrorBody)))]
/// Sets how the chain's tickets are reviewed: the mode and the reviewer agent profile. A ticket
/// that sets its own keeps it; a field left out is kept.
pub async fn patch_chain(
    State(state): State<AppState>,
    Path(chain): Path<String>,
    Json(settings): Json<ReviewSettings>,
) -> ApiResult<Json<ChainDetail>> {
    let detail = chain_write(&state, Actor::Human, move |tx| {
        Ok(reviews::set_chain(tx, &chain, settings)?)
    })
    .await?;
    Ok(Json(detail))
}

#[utoipa::path(post, path = "/chains/{chain}/edges", tag = "chains", request_body = EdgeInput,
    params(("chain" = String, Path, description = "Chain id or name")),
    responses((status = 200, body = ChainDetail), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn add_edge(
    State(state): State<AppState>,
    Path(chain): Path<String>,
    Json(e): Json<EdgeInput>,
) -> ApiResult<Json<ChainDetail>> {
    let detail = chain_write(&state, Actor::Human, move |tx| {
        Ok(chains::add_edge(tx, &chain, &e.from, &e.to, e.kind)?)
    })
    .await?;
    Ok(Json(detail))
}

#[utoipa::path(post, path = "/chains/{chain}/tickets/{ticket}/move", tag = "chains", request_body = MoveInput,
    params(("chain" = String, Path, description = "Chain id or name"), ("ticket" = String, Path, description = "Ticket key")),
    responses((status = 200, body = ChainDetail), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
/// Puts a card directly before or after another card; the dependencies around it are rewired.
pub async fn move_ticket(
    State(state): State<AppState>,
    Path((chain, ticket)): Path<(String, String)>,
    Json(m): Json<MoveInput>,
) -> ApiResult<Json<ChainDetail>> {
    let detail = chain_write(&state, Actor::Human, move |tx| {
        Ok(chains::move_ticket(
            tx, &chain, &ticket, &m.anchor, m.place,
        )?)
    })
    .await?;
    Ok(Json(detail))
}

#[utoipa::path(delete, path = "/chains/{chain}/edges", tag = "chains",
    params(("chain" = String, Path, description = "Chain id or name"), EdgeInput),
    responses((status = 200, body = ChainDetail), (status = 404, body = ErrorBody)))]
pub async fn remove_edge(
    State(state): State<AppState>,
    Path(chain): Path<String>,
    Query(e): Query<EdgeInput>,
) -> ApiResult<Json<ChainDetail>> {
    let detail = chain_write(&state, Actor::Human, move |tx| {
        Ok(chains::remove_edge(tx, &chain, &e.from, &e.to)?)
    })
    .await?;
    Ok(Json(detail))
}
