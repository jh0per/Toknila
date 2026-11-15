//! `POST /planner/activity`: the planner's process reports its turn from its hooks, with its own
//! token (`PLANNER_ROUTES`). The turn and its step live in memory beside the live run
//! (`crate::runner::Turns`, rules in `toknila_core::planner::turn`); only the turn's start and
//! end are journaled (`planner.turn`), and a step writes nothing to the database.

use std::path::Path;

use axum::Extension;
use axum::extract::State;
use axum::http::StatusCode;
use toknila_api_types::PlannerActivity;
use toknila_app::planner;
use toknila_core::planner::{self as rules, turn};
use toknila_core::{Actor, AgentRole};

use super::ApiResult;
use crate::http::body::Json;
use crate::http::error::ErrorBody;
use crate::server::{AppState, Principal, now};

#[utoipa::path(post, path = "/planner/activity", tag = "planner", request_body = PlannerActivity,
    responses((status = 204, description = "Recorded; a signal of a run that is not the project's live planner changes nothing"), (status = 400, body = ErrorBody), (status = 403, body = ErrorBody)))]
/// What the planner does now: `user_prompt_submit` starts its turn, `pre_tool_use` names its
/// step, `stop` ends the turn. `GET /projects/{project}/planner` answers the turn.
pub async fn planner_activity(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(input): Json<PlannerActivity>,
) -> ApiResult<StatusCode> {
    let (project, run) = principal.planner_run()?;
    let key = rules::run_key(project);
    // A planner that is not running is never working, and a run that is ending reports nothing.
    let Some(live) = state
        .runners
        .live_run(key, AgentRole::Planner)
        .filter(|r| r.id == run)
    else {
        return Ok(StatusCode::NO_CONTENT);
    };
    let step = turn::step(
        input.signal,
        input.tool_name.as_deref(),
        input.tool.as_deref(),
        Some(Path::new(&live.dir)),
    );
    let at = now();
    let change = state
        .runners
        .turns()
        .apply(key, run, input.signal, step, at);
    if let Some(working) = change {
        state
            .store
            .write(Actor::Agent, at, move |tx| -> ApiResult<()> {
                Ok(planner::turn(tx, project, working)?)
            })
            .await?;
    }
    Ok(StatusCode::NO_CONTENT)
}
