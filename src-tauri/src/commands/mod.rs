use std::path::PathBuf;

use tauri::{AppHandle, State};

use crate::core::{models::AppStatus, vault};
use crate::AppState;

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
