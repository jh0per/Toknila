//! HTTP server on 127.0.0.1: random port, bearer token, `/healthz`, graceful shutdown.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::StatusCode;
use axum::middleware;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use tokio::sync::Notify;
use toknila_core::Timestamp;

use crate::store::Store;

mod auth;
mod lifecycle;
mod logging;

pub(crate) use auth::constant_time_eq;
pub use auth::{AGENT_ROUTES, PLANNER_ROUTES, Principal};
pub use lifecycle::{RunError, probe, run};
pub use logging::init_logging;

/// Shared state of request handlers.
#[derive(Debug, Clone)]
pub struct AppState {
    pub store: Store,
    pub token: Arc<str>,
    pub shutdown: Arc<Notify>,
    /// The profiles folder (`Paths::profiles`); read on every request, so edits apply at once.
    pub profiles: PathBuf,
    /// The agents' terminals (`/tickets/{ticket}/runner/...`).
    pub runners: Arc<crate::runner::Runners>,
    /// How the system checks and the repository checks run `git`, `claude` and `gh`.
    pub tools: Arc<dyn crate::tools::Tools>,
    /// Token usage: transcripts, prices and the page's numbers.
    pub usage: Arc<crate::usage::Usage>,
    /// The data home (`Paths::home`): nothing in it is ever moved to the Trash.
    pub home: PathBuf,
    /// Where new ships (worktrees) are made (`Paths::ships`).
    pub ships: PathBuf,
    /// Where a deleted project's folder goes when the person asks (`SystemTrash`; tests a fake).
    pub trash: Arc<dyn crate::trash::Trasher>,
}

/// Current wall-clock time.
pub fn now() -> Timestamp {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    Timestamp(i64::try_from(ms).unwrap_or(i64::MAX))
}

#[derive(Debug, Serialize)]
struct Health {
    ok: bool,
    version: &'static str,
    pid: u32,
}

async fn healthz() -> Json<Health> {
    Json(Health {
        ok: true,
        version: toknila_core::VERSION,
        pid: std::process::id(),
    })
}

async fn shutdown(State(state): State<AppState>) -> StatusCode {
    tracing::info!("shutdown requested over the API");
    state.shutdown.notify_one();
    StatusCode::ACCEPTED
}

/// The full router. `/healthz` is open; everything under `/v1` needs the bearer token.
pub fn router(state: AppState) -> Router {
    let (api, doc) = crate::http::v1();
    let doc = Arc::new(doc);
    let v1 = api
        .route("/shutdown", post(shutdown))
        .route("/stream", get(crate::stream::stream))
        .route(
            "/tickets/{ticket}/runner/stream",
            get(crate::runner::stream),
        )
        .route(
            "/projects/{project}/planner/stream",
            get(crate::runner::planner_stream),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_token,
        ));
    Router::new()
        .route("/", get(crate::web::index))
        // The page's own routes, so a deep link opens the right screen (apps/web/src/store/route.ts).
        .route("/chains/{chain}", get(crate::web::index))
        .route("/inbox", get(crate::web::index))
        .route("/settings", get(crate::web::index))
        .route("/usage", get(crate::web::index))
        .route("/settings/{section}", get(crate::web::index))
        .route("/projects/{project}/settings", get(crate::web::index))
        .route(
            "/projects/{project}/settings/{section}",
            get(crate::web::index),
        )
        .route("/projects/{project}", get(crate::web::index))
        .route("/projects/{project}/planner", get(crate::web::index))
        .route("/projects/{project}/chains/{chain}", get(crate::web::index))
        .route(
            "/projects/{project}/chains/{chain}/cards/{key}",
            get(crate::web::index),
        )
        .route("/assets/{*path}", get(crate::web::asset))
        .route("/healthz", get(healthz))
        .route(
            "/openapi.json",
            get(move || async move { Json(doc.as_ref().clone()) }),
        )
        .nest("/v1", v1)
        .with_state(state)
}

#[cfg(test)]
mod tests;
