use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use rusqlite::Connection;
use tauri::{AppHandle, Manager};

const SCHEMA: &str = include_str!("schema.sql");

pub fn default_db_path(app: &AppHandle) -> Result<PathBuf> {
    Ok(app
        .path()
        .app_data_dir()
        .context("failed to resolve app data directory")?
        .join("index.sqlite"))
}

pub fn open(db_path: &Path) -> Result<Connection> {
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let conn = Connection::open(db_path)
        .with_context(|| format!("failed to open SQLite index at {}", db_path.display()))?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .context("failed to enable SQLite foreign keys")?;
    Ok(conn)
}

pub fn apply_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA)
        .context("failed to apply SQLite schema")?;
    Ok(())
}

pub fn initialize(db_path: &Path) -> Result<()> {
    let conn = open(db_path)?;
    apply_schema(&conn)
}
