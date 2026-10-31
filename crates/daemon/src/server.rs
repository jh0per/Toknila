//! HTTP server on 127.0.0.1: random port, bearer token, `/healthz`, graceful shutdown.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use tokio::sync::Notify;
use toknila_core::Timestamp;

use crate::info::{DaemonInfo, new_token};
use crate::paths::Paths;
use crate::store::Store;

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

async fn require_token(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let expected = format!("Bearer {}", state.token);
    let ok = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| constant_time_eq(v.as_bytes(), expected.as_bytes()));
    if ok {
        next.run(req).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "missing or wrong bearer token" })),
        )
            .into_response()
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// The full router. `/healthz` is open; everything under `/v1` needs the bearer token.
pub fn router(state: AppState) -> Router {
    let (api, doc) = crate::api::v1();
    let doc = Arc::new(doc);
    let v1 = api
        .route("/shutdown", post(shutdown))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_token));
    Router::new()
        .route("/healthz", get(healthz))
        .route(
            "/openapi.json",
            get(move || async move { Json(doc.as_ref().clone()) }),
        )
        .nest("/v1", v1)
        .with_state(state)
}

/// Daemon failures at startup.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("a daemon is already running (pid {pid}, port {port})")]
    AlreadyRunning { pid: u32, port: u16 },
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("store: {0}")]
    Store(#[from] crate::store::StoreError),
    #[error("{0}")]
    Other(String),
}

/// Is a daemon answering at the address in `daemon.json`?
pub async fn probe(info: &DaemonInfo) -> bool {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, info.port));
    tokio::time::timeout(
        std::time::Duration::from_millis(500),
        tokio::net::TcpStream::connect(addr),
    )
    .await
    .is_ok_and(|r| r.is_ok())
}

/// Runs the daemon in the foreground until Ctrl-C, SIGTERM or `POST /v1/shutdown`.
pub async fn run(paths: Paths) -> Result<(), RunError> {
    std::fs::create_dir_all(&paths.home)?;
    if let Some(info) = DaemonInfo::read(&paths.daemon_json())?
        && info.pid != std::process::id()
        && probe(&info).await
    {
        return Err(RunError::AlreadyRunning {
            pid: info.pid,
            port: info.port,
        });
    }
    let store = Store::open(&paths.db())?;
    let token: Arc<str> = new_token().map_err(RunError::Other)?.into();
    let shutdown = Arc::new(Notify::new());
    let state = AppState {
        store,
        token: token.clone(),
        shutdown: shutdown.clone(),
    };

    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();
    let info = DaemonInfo {
        pid: std::process::id(),
        port,
        token: token.to_string(),
        version: toknila_core::VERSION.to_owned(),
        started_at: now().0,
    };
    info.write(&paths.daemon_json())?;
    tracing::info!(port, pid = info.pid, "daemon listening on 127.0.0.1");

    let result = axum::serve(listener, router(state))
        .with_graceful_shutdown(async move {
            tokio::select! {
                () = shutdown.notified() => {}
                () = terminate_signal() => tracing::info!("termination signal"),
            }
        })
        .await;

    // Remove daemon.json only if it is still ours (a newer daemon may have replaced it).
    if DaemonInfo::read(&paths.daemon_json())
        .ok()
        .flatten()
        .is_some_and(|i| i.pid == info.pid)
    {
        let _ = std::fs::remove_file(paths.daemon_json());
    }
    tracing::info!("daemon stopped");
    result.map_err(RunError::Io)
}

async fn terminate_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut term) => {
                tokio::select! {
                    _ = term.recv() => {}
                    _ = tokio::signal::ctrl_c() => {}
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// Sends logs to `<home>/logs/daemon.log.<date>`, rotated daily, seven files kept.
/// Keep the returned guard alive for the life of the process.
pub fn init_logging(paths: &Paths) -> Result<tracing_appender::non_blocking::WorkerGuard, String> {
    let appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("daemon")
        .filename_suffix("log")
        .max_log_files(7)
        .build(paths.logs())
        .map_err(|e| format!("cannot open log directory: {e}"))?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let filter = tracing_subscriber::EnvFilter::try_from_env("TOKNILA_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .try_init()
        .ok();
    Ok(guard)
}

#[cfg(test)]
mod tests;
