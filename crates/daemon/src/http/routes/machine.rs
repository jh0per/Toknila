//! `/machine/settings`: what the daemon does on this machine, whichever the project.

use axum::extract::State;

use super::ApiResult;
use crate::http::body::Json;
use crate::http::dto::MachineSettings;
use crate::server::AppState;

async fn current(state: &AppState) -> ApiResult<MachineSettings> {
    Ok(MachineSettings {
        max_checks: state.store.max_checks(),
        sounds: Some(state.store.sounds().await?),
    })
}

#[utoipa::path(get, path = "/machine/settings", tag = "machine",
    responses((status = 200, body = MachineSettings)))]
/// How many hand-in checks may run at once on this machine, and which sounds the window plays.
pub async fn get_machine_settings(
    State(state): State<AppState>,
) -> ApiResult<Json<MachineSettings>> {
    Ok(Json(current(&state).await?))
}

#[utoipa::path(put, path = "/machine/settings", tag = "machine", request_body = MachineSettings,
    responses((status = 200, body = MachineSettings)))]
/// Sets how many hand-in checks may run at once (at least 1; it applies to the line at once) and,
/// when the body has them, the sound switches; all are kept across restarts. A body without
/// `sounds` (an older `tok` or page) leaves the switches as they are.
pub async fn put_machine_settings(
    State(state): State<AppState>,
    Json(input): Json<MachineSettings>,
) -> ApiResult<Json<MachineSettings>> {
    state.store.set_max_checks(input.max_checks).await?;
    if let Some(sounds) = input.sounds {
        state.store.set_sounds(sounds).await?;
    }
    Ok(Json(current(&state).await?))
}
