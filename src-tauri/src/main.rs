mod commands;
mod core;

use std::sync::Mutex;

use crate::core::vault;
use tauri::Manager;

pub struct AppState {
    vault_path: Mutex<std::path::PathBuf>,
}

impl AppState {
    fn new(vault_path: std::path::PathBuf) -> Self {
        Self {
            vault_path: Mutex::new(vault_path),
        }
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let vault_path = vault::load_or_default_vault_path(app.handle())?;
            vault::ensure_vault(&vault_path)?;
            app.manage(AppState::new(vault_path));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_status,
            commands::set_vault_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running CourseLib");
}
