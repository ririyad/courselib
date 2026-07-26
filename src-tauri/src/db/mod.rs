use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use rusqlite::Connection;
use tauri::{AppHandle, Manager};

const SCHEMA: &str = include_str!("schema.sql");
const SCHEMA_VERSION: i64 = 2;

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
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .context("failed to read SQLite schema version")?;

    // SQLite is a disposable index. Recreate old index schemas rather than carrying
    // fragile table-copy migrations; startup immediately rebuilds everything from the vault.
    if version < SCHEMA_VERSION {
        conn.execute_batch(
            "DROP TABLE IF EXISTS section_search;
             DROP TABLE IF EXISTS section_progress;
             DROP TABLE IF EXISTS course_path_items;
             DROP TABLE IF EXISTS course_paths;
             DROP TABLE IF EXISTS course_categories;
             DROP TABLE IF EXISTS categories;
             DROP TABLE IF EXISTS course_sections;
             DROP TABLE IF EXISTS courses;",
        )
        .context("failed to reset the old SQLite index")?;
    }

    apply_schema(&conn)?;
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)
        .context("failed to update SQLite schema version")?;
    Ok(())
}
