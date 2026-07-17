mod commands;
mod core;
mod db;

use std::{path::PathBuf, sync::Mutex};

use crate::core::{indexer, vault};
use tauri::Manager;

pub struct AppState {
    vault_path: Mutex<PathBuf>,
    db_path: PathBuf,
}

impl AppState {
    fn new(vault_path: PathBuf, db_path: PathBuf) -> Self {
        Self {
            vault_path: Mutex::new(vault_path),
            db_path,
        }
    }

    pub fn db_path(&self) -> &std::path::Path {
        &self.db_path
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let vault_path = vault::load_or_default_vault_path(app.handle())?;
            vault::ensure_vault(&vault_path)?;
            let db_path = db::default_db_path(app.handle())?;
            db::initialize(&db_path)?;
            let mut conn = db::open(&db_path)?;
            indexer::reindex_vault(&mut conn, &vault_path)?;
            app.manage(AppState::new(vault_path, db_path));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_status,
            commands::set_vault_path,
            commands::import_course,
            commands::delete_course,
            commands::list_courses,
            commands::get_course,
            commands::get_section,
            commands::update_course_meta,
            commands::list_categories,
            commands::create_category,
            commands::list_paths,
            commands::create_path,
            commands::get_path,
            commands::add_course_to_path,
            commands::reorder_path_items,
            commands::get_path_progress,
            commands::mark_section_status,
            commands::get_course_progress,
            commands::check_source_drift,
            commands::reimport_course,
            commands::reindex_vault
        ])
        .run(tauri::generate_context!())
        .expect("error while running CourseLib");
}
