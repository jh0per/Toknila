//! The app menu: the platform's default menu with File > Add Project... on top. The window has no
//! Tauri IPC, so the item reaches the page through the address hash it understands
//! (`#/add-project`), which opens the add-a-project form.

use tauri::menu::{Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Manager, Runtime};

const ADD_PROJECT: &str = "add-project";

/// The default menu plus "Add Project..." (Cmd+Shift+N, Ctrl+Shift+N) first in File; where the
/// platform's default menu has no File menu (Linux), one is added.
pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let menu = Menu::default(app)?;
    let add = MenuItem::with_id(
        app,
        ADD_PROJECT,
        "Add Project...",
        true,
        Some("CmdOrCtrl+Shift+N"),
    )?;
    let file = menu.items()?.into_iter().find_map(|item| {
        item.as_submenu()
            .filter(|s| s.text().is_ok_and(|t| t == "File"))
            .cloned()
    });
    match file {
        Some(file) => {
            file.prepend(&PredefinedMenuItem::separator(app)?)?;
            file.prepend(&add)?;
        }
        None => menu.prepend(&Submenu::with_items(app, "File", true, &[&add])?)?,
    }
    Ok(menu)
}

/// Opens the add-a-project form in the main window when its menu item is chosen.
pub fn on_event<R: Runtime>(app: &AppHandle<R>, id: &MenuId) {
    if id != ADD_PROJECT {
        return;
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_focus();
        let _ = window.eval("window.location.hash = '#/add-project';");
    }
}
