//! Looks for a newer Toknila, downloads it in the background and installs it when the person
//! restarts. The decisions (when, what to show, whether to warn) live in `update.rs`.
//!
//! The release server is the endpoint in `tauri.conf.json`. `TOKNILA_UPDATE_ENDPOINT` replaces it
//! (the local update test uses this). That is safe to leave in a release build: the updater only
//! installs an archive whose signature verifies against the public key compiled into the app, so
//! a changed address cannot make the app install anything its maker did not sign, and a plain
//! `http` address is refused unless the build was made with `dangerousInsecureTransportProtocol`.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::update::{self, Gate, Phase, Status, View};

/// Restarts by itself once an update is ready and no agent runs; set by the update test only. The
/// gate is the same as for the button: agents running means no restart.
const AUTO_RESTART_ENV: &str = "TOKNILA_UPDATE_AUTO_RESTART";

/// Replaces the release address (see the module doc).
const ENDPOINT_ENV: &str = "TOKNILA_UPDATE_ENDPOINT";

struct Inner {
    status: Status,
    ready: Option<(Update, Vec<u8>)>,
}

/// The shell's update state, managed by Tauri.
pub struct Updates(Mutex<Inner>);

impl Updates {
    /// Starts idle, never checked.
    pub fn new(current_version: &str) -> Self {
        Updates(Mutex::new(Inner {
            status: Status {
                current_version: current_version.to_owned(),
                last_check: None,
                phase: Phase::Idle,
                error: None,
            },
            ready: None,
        }))
    }

    fn with<T>(&self, f: impl FnOnce(&mut Inner) -> T) -> T {
        let mut g = self.0.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut g)
    }

    fn view(&self) -> View {
        self.with(|i| update::view(&i.status))
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// Checks at start and then every six hours, for as long as the app runs.
pub fn spawn_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let last = app.state::<Updates>().with(|i| i.status.last_check);
            tokio::time::sleep(update::wait_before_next(now(), last)).await;
            check(&app).await;
            if std::env::var_os(AUTO_RESTART_ENV).is_some() {
                let _ = restart(&app, false).await;
            }
        }
    });
}

/// One check, and a background download when something newer is found. Concurrent calls return
/// at once.
async fn check(app: &AppHandle) {
    let updates = app.state::<Updates>();
    let started = updates.with(|i| {
        if matches!(i.status.phase, Phase::Checking | Phase::Downloading(_)) {
            return None;
        }
        let was = std::mem::replace(&mut i.status.phase, Phase::Checking);
        Some(was)
    });
    let Some(was) = started else { return };
    let result = find_and_download(app, &was).await;
    updates.with(|i| {
        i.status.last_check = Some(now());
        match result {
            Ok(Some((u, bytes))) => {
                i.status.phase = Phase::Ready {
                    version: u.version.clone(),
                    notes: u.body.clone().unwrap_or_default(),
                };
                i.ready = Some((u, bytes));
                i.status.error = None;
            }
            Ok(None) => {
                i.status.phase = was;
                i.status.error = None;
            }
            Err(e) => {
                i.status.phase = was;
                i.status.error = Some(format!("Could not check for updates: {e}"));
            }
        }
    });
}

/// `Ok(None)` when nothing newer than what is running (or already downloaded) exists.
async fn find_and_download(
    app: &AppHandle,
    was: &Phase,
) -> Result<Option<(Update, Vec<u8>)>, String> {
    let mut builder = app.updater_builder();
    if let Ok(url) = std::env::var(ENDPOINT_ENV) {
        let url = url.parse().map_err(|e| format!("{ENDPOINT_ENV}: {e}"))?;
        builder = builder.endpoints(vec![url]).map_err(|e| e.to_string())?;
    }
    let updater = builder.build().map_err(|e| e.to_string())?;
    let Some(found) = updater.check().await.map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    if matches!(was, Phase::Ready { version, .. } if *version == found.version) {
        return Ok(None);
    }
    let updates = app.state::<Updates>();
    updates.with(|i| i.status.phase = Phase::Downloading(found.version.clone()));
    // The plugin checks the signature against the embedded public key before it hands back bytes.
    let bytes = found
        .download(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    Ok(Some((found, bytes)))
}

/// Installs the downloaded update and relaunches, unless agents are running and the person has
/// not confirmed. The relaunched app replaces the old daemon (see `takeover.rs`).
async fn restart(app: &AppHandle, confirmed: bool) -> Result<Gate, String> {
    let running = crate::toknila(app, &["ls", "--json"])
        .await
        .ok()
        .and_then(|out| update::count_running(&out));
    let gate = update::restart_gate(running, confirmed);
    if gate != Gate::Go {
        return Ok(gate);
    }
    let Some((found, bytes)) = app.state::<Updates>().with(|i| i.ready.take()) else {
        return Err("no update is ready".to_owned());
    };
    if let Err(e) = found.install(&bytes) {
        app.state::<Updates>().with(|i| {
            i.status.error = Some(format!("Could not install the update: {e}"));
            i.ready = Some((found, bytes));
        });
        return Err(format!("could not install the update: {e}"));
    }
    app.restart()
}

/// The update state for the banner and the About box.
#[tauri::command]
pub fn update_status(state: State<'_, Updates>) -> View {
    state.view()
}

/// "Check for updates": a check now, then the state.
#[tauri::command]
pub async fn check_for_updates(app: AppHandle, state: State<'_, Updates>) -> Result<View, String> {
    check(&app).await;
    Ok(state.view())
}

/// "Restart to update" (`confirmed` false) or "Restart anyway" (`confirmed` true).
#[tauri::command]
pub async fn restart_to_update(app: AppHandle, confirmed: bool) -> Result<Gate, String> {
    restart(&app, confirmed).await
}
