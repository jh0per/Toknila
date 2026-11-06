//! `/profiles`.

use axum::Json;
use axum::extract::State;

use super::ApiResult;
use crate::http::dto::ProfileList;
use crate::server::AppState;

#[utoipa::path(get, path = "/profiles", tag = "profiles",
    responses((status = 200, body = ProfileList)))]
pub async fn list_profiles(State(state): State<AppState>) -> ApiResult<Json<ProfileList>> {
    // A handful of small files: read inline, on every request, so edits apply at once.
    let catalog = crate::profiles::load(&state.profiles);
    Ok(Json(ProfileList::new(
        state.profiles.display().to_string(),
        catalog,
    )))
}
