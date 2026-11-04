//! HTTP server on 127.0.0.1: random port, bearer token, `/healthz`, graceful shutdown.

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

#[cfg(test)]
use auth::constant_time_eq;
pub use lifecycle::{RunError, probe, run};
pub use logging::init_logging;

/// Shared state of request handlers.
#[derive(Debug, Clone)]
pub struct AppState {
    pub store: Store,
    pub token: Arc<str>,
    pub shutdown: Arc<Notify>,
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
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_token,
        ));
    Router::new()
        .route("/", get(crate::web::index))
        // The page's own routes, so a deep link opens the right screen (apps/web/src/store/route.ts).
        .route("/chains/{chain}", get(crate::web::index))
        .route("/inbox", get(crate::web::index))
        .route("/projects/{project}", get(crate::web::index))
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
