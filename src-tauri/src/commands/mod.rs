use std::path::PathBuf;

use serde::Deserialize;
use tauri::{AppHandle, State};

use crate::core::{
    models::{AppStatus, WrittenCourse},
    source_fetch::{fetch_link, fetched_from_paste},
    vault,
};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub enum ImportCourseSource {
    Link {
        url: String,
    },
    Pasted {
        content: String,
        title_hint: Option<String>,
    },
}

#[tauri::command]
pub fn get_app_status(state: State<'_, AppState>) -> Result<AppStatus, String> {
    let vault_path = state
        .vault_path
        .lock()
        .map_err(|_| "vault state lock poisoned".to_string())?
        .clone();

    vault::ensure_vault(&vault_path).map_err(|err| err.to_string())?;
    Ok(vault::status(&vault_path))
}

#[tauri::command]
pub fn set_vault_path(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<AppStatus, String> {
    let vault_path = PathBuf::from(path);

    vault::ensure_vault(&vault_path).map_err(|err| err.to_string())?;
    vault::save_vault_path(&app, &vault_path).map_err(|err| err.to_string())?;

    *state
        .vault_path
        .lock()
        .map_err(|_| "vault state lock poisoned".to_string())? = vault_path.clone();

    Ok(vault::status(&vault_path))
}

#[tauri::command]
pub async fn import_course(
    state: State<'_, AppState>,
    source: ImportCourseSource,
) -> Result<WrittenCourse, String> {
    let vault_path = state
        .vault_path
        .lock()
        .map_err(|_| "vault state lock poisoned".to_string())?
        .clone();

    let fetched = match source {
        ImportCourseSource::Link { url } => {
            fetch_link(&url).await.map_err(|err| err.to_string())?
        }
        ImportCourseSource::Pasted {
            content,
            title_hint,
        } => fetched_from_paste(content, title_hint),
    };

    vault::write_fetched_course(&vault_path, fetched).map_err(|err| err.to_string())
}
