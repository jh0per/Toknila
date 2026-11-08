//! Handing the window from an older daemon to ours. When agents run on the older daemon the
//! bundled start page asks (through the two IPC commands here, allowed for the start page only,
//! see `capabilities/handover.json`): "replace now" stops the daemon and its agents, "later"
//! keeps the window on it until no agent runs, then replaces it and says so on the page.

use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager, State, WebviewWindow};

use crate::takeover::{self, Found, Question, Wait};
use crate::{page_url, toknila};

/// The older daemon the start page is asking about.
#[derive(Default)]
pub struct Pending(Mutex<Option<(Found, u32)>>);

impl Pending {
    /// Remembers the question the page will ask.
    pub fn set(&self, found: Found, agents: u32) {
        if let Ok(mut p) = self.0.lock() {
            *p = Some((found, agents));
        }
    }

    fn take(&self) -> Option<(Found, u32)> {
        self.0.lock().ok()?.take()
    }
}

/// The question for the start page, or nothing when no daemon is in the way.
#[tauri::command]
pub fn takeover_state(pending: State<'_, Pending>) -> Option<Question> {
    let p = pending.0.lock().ok()?;
    let (found, agents) = p.as_ref()?;
    Some(Question::new(found, *agents))
}

/// The person's answer, `"now"` or `"later"`; on success the window shows a daemon's page.
#[tauri::command]
pub async fn takeover_choose(
    app: AppHandle,
    window: WebviewWindow,
    pending: State<'_, Pending>,
    choice: String,
) -> Result<(), String> {
    let (found, _) = pending.take().ok_or("nothing to decide any more")?;
    match choice.as_str() {
        "now" => {
            stop_old(&app, &found).await?;
            toknila(&app, &["daemon", "start"]).await?;
        }
        "later" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move { replace_when_idle(&app).await });
        }
        other => return Err(format!("unknown answer {other}")),
    }
    let url = page_url(&app).await?;
    window.navigate(url).map_err(|e| e.to_string())
}

/// Stops the older daemon and checks that it is gone.
pub async fn stop_old(app: &AppHandle, found: &Found) -> Result<(), String> {
    // `daemon stop` asks the daemon to shut down and waits for it to exit (10 s).
    toknila(app, &["daemon", "stop"])
        .await
        .map_err(|e| takeover::stuck_message(found, &e))?;
    let after = toknila(app, &["daemon", "status", "--json"]).await?;
    if takeover::parse_status(&after).is_some() {
        return Err(takeover::stuck_message(
            found,
            "it still answers after the stop request",
        ));
    }
    Ok(())
}

/// "Later": once no agent runs, replaces the daemon with ours, points the window at it and
/// puts a notice on its page.
async fn replace_when_idle(app: &AppHandle) {
    loop {
        tokio::time::sleep(Duration::from_secs(30)).await;
        let Ok(status) = toknila(app, &["daemon", "status", "--json"]).await else {
            continue;
        };
        if takeover::wait_state(&status) == Wait::Busy {
            continue;
        }
        let swapped = async {
            toknila(app, &["daemon", "stop"]).await?;
            toknila(app, &["daemon", "start"]).await?;
            page_url(app).await
        }
        .await;
        let (script, url) = match swapped {
            Ok(url) => (takeover::REPLACED_LATER.to_owned(), Some(url)),
            Err(e) => (
                format!("Toknila could not replace the older daemon: {e}"),
                None,
            ),
        };
        if let Some(w) = app.get_webview_window("main") {
            if let Some(url) = url {
                let _ = w.navigate(url);
                // Let the new page load before the notice goes on it.
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            let _ = w.eval(takeover::notice_script(&script));
        }
        return;
    }
}
