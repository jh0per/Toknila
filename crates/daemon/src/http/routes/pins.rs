//! The assistant's pins on designs: the project's planner pins a question, the window lists a
//! project's pins and a person answers one in place.

use crate::http::body::Json;
use axum::Extension;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use toknila_api_types::{DesignPin, NewDesignPin, PinAnswer};
use toknila_app::access::{self, Action, Caller};
use toknila_app::pins::{self, NewPin};
use toknila_app::{AppError, resolve};
use toknila_core::{Actor, PinId};

use super::{ApiResult, read};
use crate::designs::Designs;
use crate::http::error::{ApiError, ErrorBody};
use crate::server::{AppState, Principal, now};

/// `caller` pins a question on a design of `project`; only the project's planner asks.
pub(super) async fn ask_as(
    state: &AppState,
    caller: Caller,
    project: String,
    input: NewDesignPin,
) -> ApiResult<DesignPin> {
    let project = read(state, move |repo| {
        let p = resolve::project(repo, &project)?;
        access::on_project(repo, &caller, &Action::Speak, p.id)?;
        Ok(p.id)
    })
    .await?;
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
        .write(caller.actor(), now(), move |tx| -> ApiResult<DesignPin> {
            Ok(pins::ask(tx, project, new)?)
        })
        .await?;
    Ok(pin)
}

#[utoipa::path(post, path = "/projects/{project}/design-pins", tag = "designs", request_body = NewDesignPin,
    params(("project" = String, Path, description = "Project id or key")),
    responses((status = 201, body = DesignPin), (status = 400, body = ErrorBody), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// The project's planner pins a question on a region of one of its designs. It shows on the
/// mockup and in the inbox until the person answers.
pub async fn ask_pin(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(project): Path<String>,
    Json(input): Json<NewDesignPin>,
) -> ApiResult<(StatusCode, Json<DesignPin>)> {
    let pin = ask_as(&state, principal.caller(), project, input).await?;
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
