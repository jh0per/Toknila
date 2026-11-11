//! `/projects/{project}/designs`: the design library of a project, and its Figma token.

use std::collections::HashMap;

use axum::Json;
use axum::body::{Body, to_bytes};
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use toknila_app::design_use::{self, DesignUse};
use toknila_app::{AppError, resolve};
use toknila_core::design::{self, DesignKind, MAX_DESIGN_BYTES};
use toknila_core::{Actor, DesignId};

use super::{ApiResult, read};
use crate::designs::{DesignError, Designs, figma};
use crate::http::dto::{Design, DesignList, FigmaToken, NewFigmaDesign, RenameDesign};
use crate::http::error::{ApiError, ErrorBody};
use crate::server::{AppState, now};

impl From<DesignError> for ApiError {
    fn from(e: DesignError) -> Self {
        Self(match e {
            DesignError::Invalid(m) => AppError::bad_request(m),
            DesignError::NotFound(d) => AppError::not_found(format!("design {d}")),
            DesignError::Io(e) => AppError::internal(e.to_string()),
        })
    }
}

async fn designs_of(state: &AppState, project: String) -> ApiResult<Designs> {
    let p = read(state, move |repo| Ok(resolve::project(repo, &project)?)).await?;
    Ok(Designs::new(&state.home, p.id))
}

fn design_id(raw: &str) -> ApiResult<DesignId> {
    raw.parse()
        .map_err(|_| ApiError(AppError::not_found(format!("design {raw}"))))
}

/// Runs a blocking disk or network step off the async threads.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, DesignError> + Send + 'static,
) -> ApiResult<T> {
    Ok(tokio::task::spawn_blocking(f).await??)
}

#[utoipa::path(get, path = "/projects/{project}/designs", tag = "designs",
    params(("project" = String, Path, description = "Project id or key")),
    responses((status = 200, body = DesignList), (status = 404, body = ErrorBody)))]
/// The project's designs, oldest first, and whether it has a Figma token.
pub async fn list_designs(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> ApiResult<Json<DesignList>> {
    let d = designs_of(&state, project).await?;
    let list = blocking(move || {
        Ok(DesignList {
            designs: d.list()?,
            figma_token: d.has_token(),
        })
    })
    .await?;
    Ok(Json(list))
}

#[utoipa::path(post, path = "/projects/{project}/designs", tag = "designs",
    params(("project" = String, Path, description = "Project id or key"),
        ("name" = String, Query, description = "The file's name")),
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses((status = 201, body = Design), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// Keeps an uploaded png, jpg, webp or pdf (the raw file as the body, its name in `name`). The
/// kind comes from the file's bytes; a pdf is kept as one design.
pub async fn upload_design(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Query(q): Query<HashMap<String, String>>,
    body: Body,
) -> ApiResult<(StatusCode, Json<Design>)> {
    let d = designs_of(&state, project).await?;
    let bytes = to_bytes(body, MAX_DESIGN_BYTES + 1).await.map_err(|_| {
        ApiError(AppError::bad_request(
            "That file is too large for a design.",
        ))
    })?;
    let name = q.get("name").cloned().unwrap_or_default();
    let at = now();
    let renderer = state.pdf_renderer.clone();
    // A pdf's first page is drawn here, on the blocking thread; no renderer: no thumbnail.
    let made = blocking(move || {
        let made = d.add_file(&name, &bytes, at)?;
        match made.kind {
            DesignKind::Pdf => d.make_thumb(made.id, &*renderer),
            _ => Ok(made),
        }
    })
    .await?;
    Ok((StatusCode::CREATED, Json(made)))
}

#[utoipa::path(post, path = "/projects/{project}/designs/figma", tag = "designs",
    params(("project" = String, Path, description = "Project id or key")),
    request_body = NewFigmaDesign,
    responses((status = 201, body = Design), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// Keeps a Figma frame link as a design. With a token saved the frame is fetched as a png and
/// kept next to the uploads; without one, or when Figma refuses, the link stays with a hint.
pub async fn add_figma_design(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Json(input): Json<NewFigmaDesign>,
) -> ApiResult<(StatusCode, Json<Design>)> {
    let d = designs_of(&state, project).await?;
    let frame = design::parse_figma(&input.url).map_err(|e| ApiError(AppError::bad_request(e)))?;
    let name = input
        .name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| format!("figma {}", frame.file_key));
    let at = now();
    let made = blocking(move || {
        let made = d.add_figma(&name, frame.clone(), at)?;
        match d.token() {
            Some(t) => fetch_into(&d, made, &t),
            // Read back, so the link carries the hint of its state.
            None => d.get(made.id),
        }
    })
    .await?;
    Ok((StatusCode::CREATED, Json(made)))
}

/// Fetches the frame of `made` and keeps the image, or the reason it failed.
fn fetch_into(d: &Designs, made: Design, token: &str) -> Result<Design, DesignError> {
    let Some(frame) = made.figma.clone() else {
        return Ok(made);
    };
    match figma::fetch_frame(token, &frame) {
        Ok(png) => d.attach_image(made.id, Ok(&png)),
        Err(why) => d.attach_image(made.id, Err(why)),
    }
}

#[utoipa::path(post, path = "/projects/{project}/designs/{design}/fetch", tag = "designs",
    params(("project" = String, Path, description = "Project id or key"),
        ("design" = String, Path, description = "Design id")),
    responses((status = 200, body = Design), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// Fetches a Figma design's frame again (after a token was saved, or a fetch failed).
pub async fn fetch_design(
    State(state): State<AppState>,
    Path((project, design)): Path<(String, String)>,
) -> ApiResult<Json<Design>> {
    let d = designs_of(&state, project).await?;
    let id = design_id(&design)?;
    let made = blocking(move || {
        let one = d.get(id)?;
        if one.figma.is_none() {
            return Err(DesignError::Invalid(
                "Only a Figma link has a frame to fetch.".into(),
            ));
        }
        let token = d.token().ok_or_else(|| {
            DesignError::Invalid("Save a Figma token in the project's settings first.".into())
        })?;
        fetch_into(&d, one, &token)
    })
    .await?;
    Ok(Json(made))
}

#[utoipa::path(get, path = "/projects/{project}/designs/{design}/file", tag = "designs",
    params(("project" = String, Path, description = "Project id or key"),
        ("design" = String, Path, description = "Design id")),
    responses((status = 200, content_type = "application/octet-stream", body = Vec<u8>), (status = 404, body = ErrorBody)))]
/// The kept file as it was stored, with its media type.
pub async fn design_file(
    State(state): State<AppState>,
    Path((project, design)): Path<(String, String)>,
) -> ApiResult<Response> {
    let d = designs_of(&state, project).await?;
    let id = design_id(&design)?;
    let (bytes, mime) = blocking(move || d.read_file(id)).await?;
    Ok(file_response(bytes, mime))
}

#[utoipa::path(patch, path = "/projects/{project}/designs/{design}", tag = "designs",
    params(("project" = String, Path, description = "Project id or key"),
        ("design" = String, Path, description = "Design id")),
    request_body = RenameDesign,
    responses((status = 200, body = Design), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// Renames a design.
pub async fn rename_design(
    State(state): State<AppState>,
    Path((project, design)): Path<(String, String)>,
    Json(input): Json<RenameDesign>,
) -> ApiResult<Json<Design>> {
    let d = designs_of(&state, project).await?;
    let id = design_id(&design)?;
    Ok(Json(blocking(move || d.rename(id, &input.name)).await?))
}

#[utoipa::path(delete, path = "/projects/{project}/designs/{design}", tag = "designs",
    params(("project" = String, Path, description = "Project id or key"),
        ("design" = String, Path, description = "Design id")),
    responses((status = 204), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
/// Removes a design and its file. A design that cards or open pins still use is refused with 409
/// (`used_by` names them) unless `force=true`: then the references and the open pins go in the
/// same transaction and each card gets a system comment saying which design went.
pub async fn delete_design(
    State(state): State<AppState>,
    Path((project, design)): Path<(String, String)>,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Response> {
    let force = matches!(q.get("force").map(String::as_str), Some("true" | "1"));
    let p = read(&state, move |repo| Ok(resolve::project(repo, &project)?)).await?;
    let d = Designs::new(&state.home, p.id);
    let id = design_id(&design)?;
    let (pid, lib) = (p.id, d.clone());
    let name = blocking(move || lib.get(id)).await?.name;
    let sentence = name.clone();
    let (refused, _) = state
        .store
        .write(
            Actor::Human,
            now(),
            move |tx| -> ApiResult<Option<DesignUse>> {
                let used = design_use::of_design(&*tx, pid, id)?;
                if used.is_free() {
                    return Ok(None);
                }
                if !force {
                    return Ok(Some(used));
                }
                design_use::release(tx, pid, id, &name)?;
                Ok(None)
            },
        )
        .await?;
    if let Some(used) = refused {
        let body = ErrorBody {
            code: "conflict".to_owned(),
            error: used.sentence(&sentence),
            field: None,
            used_by: Some(used),
        };
        return Ok((StatusCode::CONFLICT, Json(body)).into_response());
    }
    blocking(move || d.remove(id)).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[utoipa::path(get, path = "/projects/{project}/designs/{design}/thumb", tag = "designs",
    params(("project" = String, Path, description = "Project id or key"),
        ("design" = String, Path, description = "Design id")),
    responses((status = 200, content_type = "application/octet-stream", body = Vec<u8>), (status = 404, body = ErrorBody)))]
/// The picture of a design: the image itself, or the first page of a pdf when one was made;
/// 404 for a design without a picture.
pub async fn design_thumb(
    State(state): State<AppState>,
    Path((project, design)): Path<(String, String)>,
) -> ApiResult<Response> {
    let d = designs_of(&state, project).await?;
    let id = design_id(&design)?;
    let (bytes, mime) = blocking(move || d.read_thumb(id)).await?;
    Ok(file_response(bytes, mime))
}

fn file_response(bytes: Vec<u8>, mime: String) -> Response {
    (
        [
            (header::CONTENT_TYPE, mime),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_owned()),
            (header::CACHE_CONTROL, "private, no-cache".to_owned()),
        ],
        bytes,
    )
        .into_response()
}

#[utoipa::path(put, path = "/projects/{project}/figma-token", tag = "designs",
    params(("project" = String, Path, description = "Project id or key")),
    request_body = FigmaToken,
    responses((status = 204), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// Saves the project's Figma personal access token as a file under the data home's `secrets`
/// folder (readable by its owner only). It is never sent back and never written to a log.
pub async fn put_figma_token(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Json(input): Json<FigmaToken>,
) -> ApiResult<StatusCode> {
    let d = designs_of(&state, project).await?;
    blocking(move || d.set_token(&input.token)).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, path = "/projects/{project}/figma-token", tag = "designs",
    params(("project" = String, Path, description = "Project id or key")),
    responses((status = 204), (status = 404, body = ErrorBody)))]
/// Forgets the project's Figma token.
pub async fn delete_figma_token(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> ApiResult<StatusCode> {
    let d = designs_of(&state, project).await?;
    blocking(move || d.clear_token()).await?;
    Ok(StatusCode::NO_CONTENT)
}
