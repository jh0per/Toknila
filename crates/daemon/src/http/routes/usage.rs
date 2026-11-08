//! `/usage`: tokens and estimated cost, and the plan quota Claude Code reported.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use serde::Deserialize;
use toknila_core::usage::WINDOWS;
use utoipa::IntoParams;

use super::ApiResult;
use crate::http::dto::{QuotaReport, UsageSettings, UsageView};
use crate::http::error::ErrorBody;
use crate::server::{AppState, Principal};
use toknila_app::AppError;

/// Query of `GET /v1/usage`.
#[derive(Debug, Deserialize, IntoParams)]
pub struct UsageQuery {
    /// The window in days: 7 (default), 30 or 90.
    pub days: Option<u32>,
}

#[utoipa::path(get, path = "/usage", tag = "usage", params(UsageQuery),
    responses((status = 200, body = UsageView), (status = 400, body = ErrorBody)))]
/// Tokens and estimated API cost over the last 7, 30 or 90 days: totals, a series per UTC day and
/// breakdowns by model, project, chain, card and workspace. Costs are estimates at API list
/// prices; on a subscription the real cost differs.
pub async fn get_usage(
    State(state): State<AppState>,
    Query(q): Query<UsageQuery>,
) -> ApiResult<Json<UsageView>> {
    let days = q.days.unwrap_or(WINDOWS[0]);
    if !WINDOWS.contains(&days) {
        return Err(
            AppError::bad_request(format!("days must be one of {WINDOWS:?}, not {days}")).into(),
        );
    }
    Ok(Json(state.usage.view(&state.store, days).await?))
}

#[utoipa::path(put, path = "/usage/settings", tag = "usage", request_body = UsageSettings,
    responses((status = 200, body = UsageSettings)))]
/// Counts every Claude Code session on this machine (the default), or only Toknila's runs.
pub async fn put_usage_settings(
    State(state): State<AppState>,
    Json(input): Json<UsageSettings>,
) -> ApiResult<Json<UsageSettings>> {
    state
        .usage
        .set_machine_scan(&state.store, input.machine_scan)
        .await?;
    Ok(Json(input))
}

#[utoipa::path(post, path = "/usage/quota", tag = "agent", request_body = QuotaReport,
    responses((status = 204, description = "Recorded"), (status = 403, body = ErrorBody)))]
/// The plan limits an agent's status line received from Claude Code (`rate_limits`); the page
/// shows them as the remaining quota until the windows reset.
pub async fn post_usage_quota(
    State(state): State<AppState>,
    Extension(_principal): Extension<Principal>,
    Json(input): Json<QuotaReport>,
) -> ApiResult<StatusCode> {
    state.usage.report_quota(&state.store, input).await?;
    Ok(StatusCode::NO_CONTENT)
}
