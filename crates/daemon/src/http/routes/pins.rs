//! The assistant's pins on designs: the planner pins a question (`/planner/pins`), the window
//! lists a project's pins and a person answers one in place.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use toknila_api_types::{DesignPin, NewDesignPin, PinAnswer};
use toknila_app::pins::{self, NewPin};
use toknila_app::{AppError, resolve};
use toknila_core::{Actor, PinId};

use super::{ApiResult, read};
use crate::designs::Designs;
use crate::http::error::{ApiError, ErrorBody};
use crate::server::{AppState, Principal, now};

#[utoipa::path(post, path = "/planner/pins", tag = "planner", request_body = NewDesignPin,
    responses((status = 201, body = DesignPin), (status = 400, body = ErrorBody), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// Pins a question on a region of a design of the planner's project. It shows on the mockup and
/// in the inbox until the person answers.
pub async fn planner_pin(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(input): Json<NewDesignPin>,
) -> ApiResult<(StatusCode, Json<DesignPin>)> {
    let project = principal.planner_project()?;
    let designs = Designs::new(&state.home, project);
    let reference = input.design.clone();
    let found = tokio::task::spawn_blocking(move || designs.find_for_planner(&reference)).await??;
    let new = NewPin {
        design: found.id,
        design_name: found.name,
        region: input.region,
        text: input.text,
        card: input.card,
    };
    let (pin, _) = state
        .store
        .write(Actor::Agent, now(), move |tx| -> ApiResult<DesignPin> {
            Ok(pins::ask(tx, project, new)?)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(pin)))
}

#[utoipa::path(get, path = "/projects/{project}/design-pins", tag = "designs",
    params(("project" = String, Path, description = "Project id or key")),
    responses((status = 200, body = Vec<DesignPin>), (status = 404, body = ErrorBody)))]
/// The assistant's pins on the project's designs, oldest first, with the answers.
pub async fn list_pins(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> ApiResult<Json<Vec<DesignPin>>> {
    let list = read(&state, move |repo| {
        let p = resolve::project(repo, &project)?;
        Ok(pins::list(repo, p.id)?)
    })
    .await?;
    Ok(Json(list))
}

#[utoipa::path(post, path = "/projects/{project}/design-pins/{pin}/answer", tag = "designs",
    params(("project" = String, Path, description = "Project id or key"),
        ("pin" = String, Path, description = "Pin id")),
    request_body = PinAnswer,
    responses((status = 200, body = DesignPin), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// A person answers a pin (or changes the answer). The answer goes into the card's prompt under
/// `From the design`.
pub async fn answer_pin(
    State(state): State<AppState>,
    Path((project, pin)): Path<(String, String)>,
    Json(input): Json<PinAnswer>,
) -> ApiResult<Json<DesignPin>> {
    let id: PinId = pin
        .parse()
        .map_err(|_| ApiError(AppError::not_found(format!("pin {pin}"))))?;
    let (done, _) = state
        .store
        .write(Actor::Human, now(), move |tx| -> ApiResult<DesignPin> {
            let p = resolve::project(&*tx, &project)?;
            Ok(pins::answer(tx, p.id, id, &input.text)?)
        })
        .await?;
    Ok(Json(done))
}
