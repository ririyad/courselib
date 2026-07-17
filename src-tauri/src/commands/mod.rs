use std::{collections::BTreeMap, path::PathBuf};

use chrono::{SecondsFormat, Utc};
use comrak::{markdown_to_html, Options};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use tauri::{AppHandle, State};

use crate::core::{
    indexer::{self, ReindexSummary},
    models::{
        AppStatus, CourseDetail, CourseListItem, CourseProgress, ProgressStatus, SectionContent,
        SectionNode, SectionProgressEntry, WrittenCourse,
    },
    source_fetch::{fetch_link, fetched_from_paste},
    vault,
};
use crate::{db, AppState};

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

#[derive(Debug, Deserialize, Default)]
pub struct CourseListFilter {
    pub category: Option<String>,
    pub include_archived: Option<bool>,
}

#[derive(Debug, Clone)]
struct FlatSectionRow {
    id: String,
    parent_section_id: Option<String>,
    canonical_path: String,
    title: String,
    heading_level: u8,
    order_index: usize,
    status: ProgressStatus,
    completed_at: Option<String>,
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

    let written =
        vault::write_fetched_course(&vault_path, fetched).map_err(|err| err.to_string())?;

    let mut conn = open_index(&state)?;
    indexer::reindex_course(&mut conn, &vault_path, &written.slug)
        .map_err(|err| err.to_string())?;

    Ok(written)
}

#[tauri::command]
pub fn list_courses(
    state: State<'_, AppState>,
    filter: Option<CourseListFilter>,
) -> Result<Vec<CourseListItem>, String> {
    let filter = filter.unwrap_or_default();
    let conn = open_index(&state)?;
    let include_archived = filter.include_archived.unwrap_or(false);

    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.slug, c.title, c.description,
                    COALESCE(COUNT(DISTINCT cs.id), 0) AS section_count
             FROM courses c
             LEFT JOIN course_sections cs ON cs.course_id = c.id
             LEFT JOIN course_categories cc ON cc.course_id = c.id
             LEFT JOIN categories cat ON cat.id = cc.category_id
             WHERE (?1 = 1 OR c.archived_at IS NULL)
               AND (?2 IS NULL OR cat.slug = ?2)
             GROUP BY c.id, c.slug, c.title, c.description
             ORDER BY lower(c.title)",
        )
        .map_err(|err| err.to_string())?;

    let rows = stmt
        .query_map(
            params![if include_archived { 1 } else { 0 }, filter.category],
            |row| {
                let id: String = row.get(0)?;
                let progress =
                    load_course_progress(&conn, &id).unwrap_or_else(|_| empty_progress());
                Ok(CourseListItem {
                    categories: load_course_categories(&conn, &id).unwrap_or_default(),
                    id,
                    slug: row.get(1)?,
                    title: row.get(2)?,
                    description: row.get(3)?,
                    section_count: row.get::<_, i64>(4)? as usize,
                    progress,
                })
            },
        )
        .map_err(|err| err.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;

    Ok(rows)
}

#[tauri::command]
pub fn get_course(state: State<'_, AppState>, course_id: String) -> Result<CourseDetail, String> {
    let conn = open_index(&state)?;
    let course = conn
        .query_row(
            "SELECT id, slug, title, description FROM courses WHERE id = ?1 OR slug = ?1",
            params![course_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .optional()
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "course not found".to_string())?;

    let rows = load_section_rows(&conn, &course.0)?;
    let sections = build_section_tree(&rows, None);
    let categories = load_course_categories(&conn, &course.0).map_err(|err| err.to_string())?;
    let progress = load_course_progress(&conn, &course.0).map_err(|err| err.to_string())?;

    Ok(CourseDetail {
        id: course.0,
        slug: course.1,
        title: course.2,
        description: course.3,
        categories,
        progress,
        sections,
    })
}

#[tauri::command]
pub fn get_section(
    state: State<'_, AppState>,
    section_id: String,
) -> Result<SectionContent, String> {
    let conn = open_index(&state)?;
    let row = conn
        .query_row(
            "SELECT cs.id, cs.course_id, cs.canonical_path, cs.title, cs.vault_path
             FROM course_sections cs
             WHERE cs.id = ?1",
            params![section_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "section not found".to_string())?;

    let raw_markdown = std::fs::read_to_string(&row.4).map_err(|err| err.to_string())?;
    let html = markdown_to_html(&raw_markdown, &markdown_options());

    Ok(SectionContent {
        id: row.0,
        course_id: row.1,
        canonical_path: row.2,
        title: row.3,
        raw_markdown,
        html,
    })
}

#[tauri::command]
pub fn mark_section_status(
    state: State<'_, AppState>,
    section_id: String,
    status: ProgressStatus,
) -> Result<CourseProgress, String> {
    let vault_path = state
        .vault_path
        .lock()
        .map_err(|_| "vault state lock poisoned".to_string())?
        .clone();

    let mut conn = open_index(&state)?;
    let (course_id, course_slug, course_vault_path, canonical_path) = conn
        .query_row(
            "SELECT c.id, c.slug, c.vault_path, cs.canonical_path
             FROM course_sections cs
             JOIN courses c ON c.id = cs.course_id
             WHERE cs.id = ?1",
            params![section_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "section not found".to_string())?;

    let progress_path = PathBuf::from(course_vault_path).join("_progress.yaml");
    write_progress_entry(&progress_path, &canonical_path, status).map_err(|err| err.to_string())?;

    indexer::reindex_course(&mut conn, &vault_path, &course_slug).map_err(|err| err.to_string())?;
    load_course_progress(&conn, &course_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_course_progress(
    state: State<'_, AppState>,
    course_id: String,
) -> Result<CourseProgress, String> {
    let conn = open_index(&state)?;
    let indexed_course_id = conn
        .query_row(
            "SELECT id FROM courses WHERE id = ?1 OR slug = ?1",
            params![course_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "course not found".to_string())?;

    load_course_progress(&conn, &indexed_course_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn reindex_vault(state: State<'_, AppState>) -> Result<ReindexSummary, String> {
    let vault_path = state
        .vault_path
        .lock()
        .map_err(|_| "vault state lock poisoned".to_string())?
        .clone();

    vault::ensure_vault(&vault_path).map_err(|err| err.to_string())?;
    let mut conn = open_index(&state)?;
    indexer::reindex_vault(&mut conn, &vault_path).map_err(|err| err.to_string())
}

fn open_index(state: &State<'_, AppState>) -> Result<Connection, String> {
    let conn = db::open(state.db_path()).map_err(|err| err.to_string())?;
    db::apply_schema(&conn).map_err(|err| err.to_string())?;
    Ok(conn)
}

fn load_course_categories(conn: &Connection, course_id: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT cat.slug
         FROM categories cat
         JOIN course_categories cc ON cc.category_id = cat.id
         WHERE cc.course_id = ?1
         ORDER BY cat.name",
    )?;
    let categories = stmt
        .query_map(params![course_id], |row| row.get::<_, String>(0))?
        .collect();
    categories
}

fn load_section_rows(conn: &Connection, course_id: &str) -> Result<Vec<FlatSectionRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT cs.id, cs.parent_section_id, cs.canonical_path, cs.title, cs.heading_level,
                    cs.order_index, COALESCE(sp.status, 'not_started'), sp.completed_at
             FROM course_sections cs
             LEFT JOIN section_progress sp ON sp.section_id = cs.id
             WHERE cs.course_id = ?1
             ORDER BY cs.parent_section_id, cs.order_index",
        )
        .map_err(|err| err.to_string())?;

    let rows = stmt
        .query_map(params![course_id], |row| {
            Ok(FlatSectionRow {
                id: row.get(0)?,
                parent_section_id: row.get(1)?,
                canonical_path: row.get(2)?,
                title: row.get(3)?,
                heading_level: row.get::<_, i64>(4)? as u8,
                order_index: row.get::<_, i64>(5)? as usize,
                status: progress_status_from_str(&row.get::<_, String>(6)?),
                completed_at: row.get(7)?,
            })
        })
        .map_err(|err| err.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string());
    rows
}

fn build_section_tree(rows: &[FlatSectionRow], parent_id: Option<&str>) -> Vec<SectionNode> {
    let mut children = rows
        .iter()
        .filter(|row| row.parent_section_id.as_deref() == parent_id)
        .cloned()
        .collect::<Vec<_>>();
    children.sort_by_key(|row| row.order_index);

    children
        .into_iter()
        .map(|row| SectionNode {
            children: build_section_tree(rows, Some(&row.id)),
            id: row.id,
            canonical_path: row.canonical_path,
            title: row.title,
            heading_level: row.heading_level,
            order_index: row.order_index,
            status: row.status,
            completed_at: row.completed_at,
        })
        .collect()
}

fn load_course_progress(conn: &Connection, course_id: &str) -> rusqlite::Result<CourseProgress> {
    let (total, not_started, in_progress, completed) = conn.query_row(
        "SELECT COUNT(cs.id),
                COALESCE(SUM(CASE WHEN COALESCE(sp.status, 'not_started') = 'not_started' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN sp.status = 'in_progress' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN sp.status = 'completed' THEN 1 ELSE 0 END), 0)
         FROM course_sections cs
         LEFT JOIN section_progress sp ON sp.section_id = cs.id
         WHERE cs.course_id = ?1",
        params![course_id],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        },
    )?;

    Ok(course_progress_from_counts(
        total as usize,
        not_started as usize,
        in_progress as usize,
        completed as usize,
    ))
}

fn empty_progress() -> CourseProgress {
    course_progress_from_counts(0, 0, 0, 0)
}

fn course_progress_from_counts(
    total_sections: usize,
    not_started: usize,
    in_progress: usize,
    completed: usize,
) -> CourseProgress {
    let percent_complete = if total_sections == 0 {
        0.0
    } else {
        (completed as f64 / total_sections as f64) * 100.0
    };

    CourseProgress {
        total_sections,
        not_started,
        in_progress,
        completed,
        percent_complete,
    }
}

fn write_progress_entry(
    path: &std::path::Path,
    canonical_path: &str,
    status: ProgressStatus,
) -> anyhow::Result<()> {
    let mut progress: BTreeMap<String, SectionProgressEntry> = if path.is_file() {
        let contents = std::fs::read_to_string(path)?;
        serde_yaml::from_str(&contents)?
    } else {
        BTreeMap::new()
    };

    if status == ProgressStatus::NotStarted {
        progress.remove(canonical_path);
    } else {
        let completed_at = (status == ProgressStatus::Completed)
            .then(|| Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true));
        progress.insert(
            canonical_path.to_string(),
            SectionProgressEntry {
                status,
                completed_at,
            },
        );
    }

    let yaml = if progress.is_empty() {
        "{}\n".to_string()
    } else {
        serde_yaml::to_string(&progress)?
    };
    std::fs::write(path, yaml)?;
    Ok(())
}

fn progress_status_from_str(value: &str) -> ProgressStatus {
    match value {
        "completed" => ProgressStatus::Completed,
        "in_progress" => ProgressStatus::InProgress,
        _ => ProgressStatus::NotStarted,
    }
}

fn markdown_options<'a>() -> Options<'a> {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.tasklist = true;
    options.extension.strikethrough = true;
    options.extension.autolink = true;
    options.render.unsafe_ = true;
    options
}
