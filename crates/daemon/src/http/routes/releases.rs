//! `/chains/{chain}/releases`, `/releases`.

use axum::Json;
use axum::extract::{Path, State};
use toknila_app::releases::{self, ReleaseAction};
use toknila_app::resolve;
use toknila_core::Actor;

use super::{ApiResult, chain_write};
use crate::http::dto::{ChainDetail, NewRelease};
use crate::http::error::ErrorBody;
use crate::server::AppState;

#[utoipa::path(post, path = "/chains/{chain}/releases", tag = "releases", request_body = NewRelease,
    params(("chain" = String, Path, description = "Chain id or name")),
    responses((status = 200, body = ChainDetail), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn create_release(
    State(state): State<AppState>,
    Path(chain): Path<String>,
    Json(input): Json<NewRelease>,
) -> ApiResult<Json<ChainDetail>> {
    let detail = chain_write(&state, Actor::Human, move |tx| {
        Ok(releases::create(tx, &chain, input)?)
    })
    .await?;
    Ok(Json(detail))
}

#[utoipa::path(post, path = "/releases/{release}/actions/{action}", tag = "releases",
    params(("release" = String, Path, description = "Release id or version"), ("action" = String, Path, description = "start (cut) or complete (released)")),
    responses((status = 200, body = ChainDetail), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn release_action(
    State(state): State<AppState>,
    Path((release, action)): Path<(String, String)>,
) -> ApiResult<Json<ChainDetail>> {
    let action: ReleaseAction = action.parse()?;
    let detail = chain_write(&state, Actor::Human, move |tx| {
        let r = resolve::release(&*tx, &release)?;
        releases::act(tx, &r, action)?;
        Ok(r.chain_id)
    })
    .await?;
    Ok(Json(detail))
}
