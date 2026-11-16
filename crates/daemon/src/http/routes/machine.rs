//! `/machine/settings`: what the daemon does on this machine, whichever the project.

use axum::extract::State;

use super::ApiResult;
use crate::http::body::Json;
use crate::http::dto::MachineSettings;
use crate::server::AppState;

#[utoipa::path(get, path = "/machine/settings", tag = "machine",
    responses((status = 200, body = MachineSettings)))]
/// How many hand-in checks may run at once on this machine.
pub async fn get_machine_settings(State(state): State<AppState>) -> Json<MachineSettings> {
    Json(MachineSettings {
        max_checks: state.store.max_checks(),
    })
}

#[utoipa::path(put, path = "/machine/settings", tag = "machine", request_body = MachineSettings,
    responses((status = 200, body = MachineSettings)))]
/// Sets how many hand-in checks may run at once (at least 1); it applies to the line at once and
/// is kept across restarts.
pub async fn put_machine_settings(
    State(state): State<AppState>,
    Json(input): Json<MachineSettings>,
) -> ApiResult<Json<MachineSettings>> {
    let max_checks = state.store.set_max_checks(input.max_checks).await?;
    Ok(Json(MachineSettings { max_checks }))
}
