//! `/usage`: tokens and estimated cost, and the plan quota Claude Code reported.

use crate::http::body::Json;
use axum::Extension;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::Deserialize;
use toknila_core::usage::WINDOWS;
use utoipa::IntoParams;

use super::ApiResult;
use crate::http::dto::{QuotaReport, TodayUsage, UsageSettings, UsageView};
use crate::http::error::ErrorBody;
use crate::server::{AppState, Principal};
use toknila_app::AppError;

/// Query of `GET /v1/usage`.
#[derive(Debug, Deserialize, IntoParams)]
pub struct UsageQuery {
    /// The window in days: 7 (default), 30 or 90.
    pub days: Option<u32>,
    /// Minutes the person's clock is east of UTC (negative to the west); the days of the series
    /// are theirs. UTC when absent.
    pub tz: Option<i32>,
}

/// The widest offset a clock can have, in minutes (UTC+14 and UTC-12 are the extremes).
fn offset(tz: Option<i32>) -> Result<i32, AppError> {
    let tz = tz.unwrap_or(0);
    if (-14 * 60..=14 * 60).contains(&tz) {
        Ok(tz)
    } else {
        Err(AppError::bad_request(format!(
            "tz is minutes east of UTC, between -840 and 840, not {tz}"
        )))
    }
}

/// Query of `GET /v1/projects/{key}/usage/today`.
#[derive(Debug, Deserialize, IntoParams)]
pub struct TodayQuery {
    /// Minutes the person's clock is east of UTC (negative to the west); UTC when absent.
    pub tz: Option<i32>,
}

#[utoipa::path(get, path = "/projects/{key}/usage/today", tag = "usage",
    params(("key" = String, Path, description = "The project's key"), TodayQuery),
    responses((status = 200, body = TodayUsage), (status = 404, body = ErrorBody),
        (status = 400, body = ErrorBody)))]
/// What today cost, by the person's local day: the project's Toknila runs, the whole machine, the
/// plan windows when a status line reported them within the hour, and the project's spend by card.
/// Estimates at API list prices; on a subscription the real cost differs. Kept for a minute and
/// dropped when a run ends.
pub async fn get_usage_today(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(q): Query<TodayQuery>,
) -> ApiResult<Json<TodayUsage>> {
    let tz = offset(q.tz)?;
    let lookup = key.clone();
    let found = state
        .store
        .read(move |repo| repo.project_by_key(&lookup))
        .await?;
    if found.is_none() {
        return Err(AppError::not_found(format!("project {key}")).into());
    }
    Ok(Json(state.usage.today(&state.store, &key, tz).await?))
}

#[utoipa::path(get, path = "/usage", tag = "usage", params(UsageQuery),
    responses((status = 200, body = UsageView), (status = 400, body = ErrorBody)))]
/// Tokens and estimated API cost over the last 7, 30 or 90 days: totals, a series per day (UTC,
/// or the person's own with `tz`) and breakdowns by model, project, chain, card and workspace.
/// Costs are estimates at API list prices; on a subscription the real cost differs.
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
    let tz = offset(q.tz)?;
    Ok(Json(state.usage.view(&state.store, days, tz).await?))
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
