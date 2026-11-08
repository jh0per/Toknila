//! The folder picker behind the Add project form's Browse button. The page asks for it through
//! Tauri IPC (`pick_folder`, allowed only for the daemon's loopback page, see
//! `capabilities/folders.json`); the dialog plugin's own commands stay out of the page's reach.

use tauri::WebviewWindow;
use tauri_plugin_dialog::DialogExt;

/// Shows the system's folder picker over the window and answers the chosen folder's path, or
/// nothing when the person cancels (or the choice is not a plain path).
#[tauri::command]
pub async fn pick_folder(window: WebviewWindow) -> Option<String> {
    let picked = tauri::async_runtime::spawn_blocking(move || {
        window
            .dialog()
            .file()
            .set_title("Choose the folder of your project")
            .set_parent(&window)
            .blocking_pick_folder()
    })
    .await
    .ok()??;
    let path = picked.into_path().ok()?;
    Some(path.display().to_string())
}
