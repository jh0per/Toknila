//! Toknila desktop shell (Tauri 2). It starts the daemon through the bundled `toknila` binary,
//! or attaches to the one already running (the same `daemon start` the CLI uses), then opens a
//! window on the daemon's web page. Closing the window leaves the daemon and its agents running.

// No console window next to the app on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod path;
mod takeover;

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_shell::ShellExt;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = open(&handle).await {
                    show_error(&handle, &e);
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("the Tauri runtime starts");
}

/// Starts or attaches to the daemon and opens the main window on its page.
async fn open(app: &AppHandle) -> Result<(), String> {
    make_way_for_our_daemon(app).await?;
    toknila(app, &["daemon", "start"]).await?;
    let open = toknila(app, &["open", "--json"]).await?;
    let url = serde_json::from_str::<serde_json::Value>(&open)
        .ok()
        .and_then(|v| v["url"].as_str().map(str::to_owned))
        .ok_or_else(|| format!("unexpected answer from `toknila open`: {open}"))?;
    let url = url
        .parse()
        .map_err(|e| format!("bad page address {url}: {e}"))?;
    WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
        .title("Toknila")
        .inner_size(1200.0, 800.0)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// The path of the bundled daemon, resolved the way the daemon reports its own.
fn own_daemon_exe() -> Option<String> {
    let exe = std::env::current_exe().ok()?.parent()?.join("toknila");
    let exe = exe.canonicalize().unwrap_or(exe);
    Some(exe.display().to_string())
}

/// A daemon of another version or from another binary (a replaced app) is stopped, so the app
/// runs its own; the same version from the same binary is left to `daemon start` to attach to.
/// If the old one does not stop, the window says so instead of attaching to it.
async fn make_way_for_our_daemon(app: &AppHandle) -> Result<(), String> {
    let status = toknila(app, &["daemon", "status", "--json"]).await?;
    let found = takeover::parse_status(&status);
    let own_exe = own_daemon_exe().unwrap_or_default();
    if takeover::decide(found.as_ref(), env!("CARGO_PKG_VERSION"), &own_exe)
        != takeover::Decision::Replace
    {
        return Ok(());
    }
    let Some(found) = found else {
        return Ok(());
    };
    // `daemon stop` asks the daemon to shut down and waits for it to exit (10 s).
    toknila(app, &["daemon", "stop"])
        .await
        .map_err(|e| takeover::stuck_message(&found, &e))?;
    let after = toknila(app, &["daemon", "status", "--json"]).await?;
    if takeover::parse_status(&after).is_some() {
        return Err(takeover::stuck_message(
            &found,
            "it still answers after the stop request",
        ));
    }
    Ok(())
}

/// Runs the bundled `toknila` binary and returns its standard output. A daemon it starts gets the
/// person's `PATH`, so it finds `claude`, `git` and `gh` although Finder gave the app a bare one.
async fn toknila(app: &AppHandle, args: &[&str]) -> Result<String, String> {
    let out = app
        .shell()
        .sidecar("toknila")
        .map_err(|e| e.to_string())?
        .env("PATH", path::for_daemon())
        .args(args)
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_owned());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Shows why the daemon could not be reached in the bundled start page.
fn show_error(app: &AppHandle, message: &str) {
    let window = match app.get_webview_window("main") {
        Some(w) => Ok(w),
        None => WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
            .title("Toknila")
            .build(),
    };
    if let Ok(w) = window {
        let text = serde_json::to_string(message).unwrap_or_default();
        let _ = w.eval(format!(
            "document.getElementById('status').textContent = {text};"
        ));
    }
}
