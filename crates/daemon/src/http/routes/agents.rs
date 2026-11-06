//! `/agents`.

use axum::Json;
use axum::extract::State;
use toknila_app::schedule;

use super::{ApiResult, read};
use crate::http::dto::ProjectAgents;
use crate::server::AppState;

#[utoipa::path(get, path = "/agents", tag = "projects",
    responses((status = 200, body = Vec<ProjectAgents>)))]
/// The agent slots of every project and the cards waiting for one, as core's schedule computes
/// them.
pub async fn list_agents(State(state): State<AppState>) -> ApiResult<Json<Vec<ProjectAgents>>> {
    let agents = read(&state, |repo| {
        Ok(schedule::plans(repo)?
            .iter()
            .map(|(project, plan)| ProjectAgents::new(project, plan))
            .collect())
    })
    .await?;
    Ok(Json(agents))
}
