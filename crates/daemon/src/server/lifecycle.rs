//! Daemon lifecycle: start (or refuse when one runs), serve, stop on a signal or request.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use tokio::sync::Notify;

use super::{AppState, now, router};
use crate::info::{DaemonInfo, new_token};
use crate::paths::Paths;
use crate::store::Store;

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
    let previous = DaemonInfo::read(&paths.daemon_json())?;
    let reuse = match &previous {
        Some(p) => Some(p.clone()),
        None => DaemonInfo::read(&paths.daemon_last_json()).ok().flatten(),
    };
    if let Some(info) = &previous
        && info.pid != std::process::id()
        && probe(info).await
    {
        return Err(RunError::AlreadyRunning {
            pid: info.pid,
            port: info.port,
        });
    }
    let (runners, ended) = crate::runner::Runners::new(paths.runs(), paths.profiles());
    let store = Store::open(&paths.db())?
        .with_shipyard(Arc::new(crate::shipyard::GitShipyard::new(paths.ships())))
        .with_merger(Arc::new(crate::github::Mergers::new()))
        .with_runner(runners.clone());
    crate::runner::supervise(store.clone(), ended);
    crate::runner::watch_silence(store.clone(), runners.clone());
    crate::pr_watch::watch(store.clone());
    let usage = Arc::new(crate::usage::Usage::new(&paths.home));
    crate::usage::watch(store.clone(), usage.clone());
    let recovery = crate::recovery::recover(&store)
        .await
        .map_err(|e| RunError::Other(format!("startup recovery: {e}")))?;
    if recovery.drift.is_empty() {
        tracing::info!(?recovery, "store checked");
    } else {
        tracing::warn!(?recovery, "stored state differs from the journal");
    }
    // A restart keeps the address and token when it can, so an open page reconnects by itself.
    let token: Arc<str> = match reuse.as_ref().filter(|i| i.token.len() >= 32) {
        Some(i) => i.token.clone().into(),
        None => new_token().map_err(RunError::Other)?.into(),
    };
    let shutdown = Arc::new(Notify::new());
    let state = AppState {
        store,
        token: token.clone(),
        shutdown: shutdown.clone(),
        profiles: paths.profiles(),
        runners: runners.clone(),
        tools: Arc::new(crate::tools::SystemTools),
        usage,
        home: paths.home.clone(),
        trash: Arc::new(crate::trash::SystemTrash),
    };

    let listener = match reuse.as_ref().map(|i| i.port).filter(|p| *p != 0) {
        Some(p) => match tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, p)).await {
            Ok(l) => l,
            Err(_) => tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?,
        },
        None => tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?,
    };
    let port = listener.local_addr()?.port();
    let info = DaemonInfo {
        pid: std::process::id(),
        port,
        token: token.to_string(),
        version: toknila_core::VERSION.to_owned(),
        started_at: now().0,
        exe: std::env::current_exe()
            .ok()
            .map(|p| p.display().to_string()),
    };
    runners.set_daemon_url(info.base_url());
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

    // Agents do not outlive the daemon that started them; the next start marks their tickets
    // `runner_lost`.
    runners.stop_all();

    // Remove daemon.json only if it is still ours (a newer daemon may have replaced it).
    if DaemonInfo::read(&paths.daemon_json())
        .ok()
        .flatten()
        .is_some_and(|i| i.pid == info.pid)
    {
        // Kept aside so the next start can take the same address and token.
        if std::fs::rename(paths.daemon_json(), paths.daemon_last_json()).is_err() {
            let _ = std::fs::remove_file(paths.daemon_json());
        }
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
