/// The commands the page may call through Tauri IPC (see `capabilities/updates.json`).
const COMMANDS: &[&str] = &["update_status", "check_for_updates", "restart_to_update"];

fn main() {
    let app = tauri_build::AppManifest::new().commands(COMMANDS);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(app))
        .expect("the Tauri build step runs");
}
