use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
};

use chrono::{SecondsFormat, Utc};
use comrak::{markdown_to_html, Options};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::core::{
    git_vault,
    indexer::{self, ReindexSummary},
    models::{
        AppStatus, Category, CourseDetail, CourseListItem, CourseManifest, CoursePathDetail,
        CoursePathItem, CoursePathSummary, CourseProgress, ProgressStatus, ReimportCourseResult,
        SectionContent, SectionNode, SectionProgressEntry, SourceDriftStatus, WrittenCourse,
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

#[derive(Debug, Deserialize, Default)]
pub struct CourseMetaPatch {
    pub title: Option<String>,
    pub description: Option<String>,
    pub categories: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct PathOrderingItem {
    pub course_id: String,
    pub optional: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
struct PathManifest {
    title: String,
    slug: String,
    description: Option<String>,
    courses: Vec<PathCourseManifest>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PathCourseManifest {
    slug: String,
    optional: bool,
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

#[derive(Debug, Clone)]
struct CourseSourceIdentity {
    id: String,
    slug: String,
    vault_path: String,
    origin_url: Option<String>,
    content_hash: Option<String>,
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
    load_course_detail(&conn, &course_id)
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
pub fn list_categories(state: State<'_, AppState>) -> Result<Vec<Category>, String> {
    let conn = open_index(&state)?;
    let mut stmt = conn
        .prepare("SELECT slug, name FROM categories ORDER BY lower(name)")
        .map_err(|err| err.to_string())?;

    let categories = stmt
        .query_map([], |row| {
            Ok(Category {
                slug: row.get(0)?,
                name: row.get(1)?,
            })
        })
        .map_err(|err| err.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;

    Ok(categories)
}

#[tauri::command]
pub fn create_category(state: State<'_, AppState>, name: String) -> Result<Category, String> {
    let vault_path = state
        .vault_path
        .lock()
        .map_err(|_| "vault state lock poisoned".to_string())?
        .clone();
    vault::ensure_vault(&vault_path).map_err(|err| err.to_string())?;

    let name = name.trim();
    if name.is_empty() {
        return Err("category name cannot be empty".to_string());
    }

    let categories_path = vault_path.join("categories.yaml");
    let mut categories = read_categories_file(&categories_path).map_err(|err| err.to_string())?;
    let slug = unique_category_slug(name, &categories);
    let category = Category {
        slug,
        name: name.to_string(),
    };
    categories.push(category.clone());
    write_categories_file(&categories_path, &categories).map_err(|err| err.to_string())?;

    let mut conn = open_index(&state)?;
    indexer::reindex_vault(&mut conn, &vault_path).map_err(|err| err.to_string())?;
    Ok(category)
}

#[tauri::command]
pub fn update_course_meta(
    state: State<'_, AppState>,
    course_id: String,
    patch: CourseMetaPatch,
) -> Result<CourseDetail, String> {
    let vault_path = state
        .vault_path
        .lock()
        .map_err(|_| "vault state lock poisoned".to_string())?
        .clone();

    let mut conn = open_index(&state)?;
    let (indexed_course_id, course_slug, course_vault_path) = conn
        .query_row(
            "SELECT id, slug, vault_path FROM courses WHERE id = ?1 OR slug = ?1",
            params![course_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "course not found".to_string())?;

    let manifest_path = PathBuf::from(course_vault_path).join("_course.yaml");
    let mut manifest: CourseManifest =
        read_yaml_file(&manifest_path).map_err(|err| err.to_string())?;

    if let Some(title) = patch.title {
        let title = title.trim();
        if title.is_empty() {
            return Err("course title cannot be empty".to_string());
        }
        manifest.title = title.to_string();
    }
    if let Some(description) = patch.description {
        manifest.description = if description.trim().is_empty() {
            None
        } else {
            Some(description)
        };
    }
    if let Some(categories) = patch.categories {
        manifest.categories =
            normalize_course_categories(&vault_path, categories).map_err(|err| err.to_string())?;
    }

    write_yaml_file(&manifest_path, &manifest).map_err(|err| err.to_string())?;
    indexer::reindex_course(&mut conn, &vault_path, &course_slug).map_err(|err| err.to_string())?;
    load_course_detail(&conn, &indexed_course_id)
}

#[tauri::command]
pub fn list_paths(state: State<'_, AppState>) -> Result<Vec<CoursePathSummary>, String> {
    let conn = open_index(&state)?;
    let mut stmt = conn
        .prepare(
            "SELECT cp.id, cp.slug, cp.title, COUNT(cpi.course_id)
             FROM course_paths cp
             LEFT JOIN course_path_items cpi ON cpi.course_path_id = cp.id
             GROUP BY cp.id, cp.slug, cp.title
             ORDER BY lower(cp.title)",
        )
        .map_err(|err| err.to_string())?;

    let paths = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let progress = load_path_progress(&conn, &id).unwrap_or_else(|_| empty_progress());
            Ok(CoursePathSummary {
                id,
                slug: row.get(1)?,
                title: row.get(2)?,
                course_count: row.get::<_, i64>(3)? as usize,
                progress,
            })
        })
        .map_err(|err| err.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;

    Ok(paths)
}

#[tauri::command]
pub fn create_path(state: State<'_, AppState>, title: String) -> Result<CoursePathSummary, String> {
    let vault_path = state
        .vault_path
        .lock()
        .map_err(|_| "vault state lock poisoned".to_string())?
        .clone();
    vault::ensure_vault(&vault_path).map_err(|err| err.to_string())?;

    let title = title.trim();
    if title.is_empty() {
        return Err("path title cannot be empty".to_string());
    }

    let slug = unique_path_slug(&vault_path, title);
    let manifest = PathManifest {
        title: title.to_string(),
        slug: slug.clone(),
        description: None,
        courses: Vec::new(),
    };
    write_yaml_file(
        &vault_path.join("paths").join(format!("{slug}.yaml")),
        &manifest,
    )
    .map_err(|err| err.to_string())?;

    let mut conn = open_index(&state)?;
    indexer::reindex_vault(&mut conn, &vault_path).map_err(|err| err.to_string())?;
    load_path_summary(&conn, &slug)
}

#[tauri::command]
pub fn get_path(state: State<'_, AppState>, path_id: String) -> Result<CoursePathDetail, String> {
    let conn = open_index(&state)?;
    load_path_detail(&conn, &path_id)
}

#[tauri::command]
pub fn add_course_to_path(
    state: State<'_, AppState>,
    path_id: String,
    course_id: String,
    order_index: Option<usize>,
    optional: Option<bool>,
) -> Result<CoursePathDetail, String> {
    let vault_path = state
        .vault_path
        .lock()
        .map_err(|_| "vault state lock poisoned".to_string())?
        .clone();
    let mut conn = open_index(&state)?;
    let (indexed_path_id, path_slug, path_vault_path) = load_path_identity(&conn, &path_id)?;
    let course_slug = conn
        .query_row(
            "SELECT slug FROM courses WHERE id = ?1 OR slug = ?1",
            params![course_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "course not found".to_string())?;

    let mut manifest = read_path_manifest(&PathBuf::from(path_vault_path))?;
    manifest.courses.retain(|course| course.slug != course_slug);
    let insert_at = order_index
        .unwrap_or(manifest.courses.len())
        .min(manifest.courses.len());
    manifest.courses.insert(
        insert_at,
        PathCourseManifest {
            slug: course_slug,
            optional: optional.unwrap_or(false),
        },
    );
    write_yaml_file(
        &vault_path.join("paths").join(format!("{path_slug}.yaml")),
        &manifest,
    )
    .map_err(|err| err.to_string())?;

    indexer::reindex_vault(&mut conn, &vault_path).map_err(|err| err.to_string())?;
    load_path_detail(&conn, &indexed_path_id)
}

#[tauri::command]
pub fn reorder_path_items(
    state: State<'_, AppState>,
    path_id: String,
    ordering: Vec<PathOrderingItem>,
) -> Result<CoursePathDetail, String> {
    let vault_path = state
        .vault_path
        .lock()
        .map_err(|_| "vault state lock poisoned".to_string())?
        .clone();
    let mut conn = open_index(&state)?;
    let (indexed_path_id, path_slug, path_vault_path) = load_path_identity(&conn, &path_id)?;
    let mut manifest = read_path_manifest(&PathBuf::from(path_vault_path))?;
    let current = manifest
        .courses
        .iter()
        .map(|course| (course.slug.clone(), course.optional))
        .collect::<BTreeMap<_, _>>();
    let mut seen = HashSet::new();
    let mut courses = Vec::new();

    for item in ordering {
        let course_slug = conn
            .query_row(
                "SELECT slug FROM courses WHERE id = ?1 OR slug = ?1",
                params![item.course_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|err| err.to_string())?
            .ok_or_else(|| "course not found".to_string())?;
        if !current.contains_key(&course_slug) {
            return Err(format!("course is not in path: {course_slug}"));
        }
        if seen.insert(course_slug.clone()) {
            courses.push(PathCourseManifest {
                slug: course_slug.clone(),
                optional: item.optional.unwrap_or(current[&course_slug]),
            });
        }
    }

    manifest.courses = courses;
    write_yaml_file(
        &vault_path.join("paths").join(format!("{path_slug}.yaml")),
        &manifest,
    )
    .map_err(|err| err.to_string())?;

    indexer::reindex_vault(&mut conn, &vault_path).map_err(|err| err.to_string())?;
    load_path_detail(&conn, &indexed_path_id)
}

#[tauri::command]
pub fn get_path_progress(
    state: State<'_, AppState>,
    path_id: String,
) -> Result<CourseProgress, String> {
    let conn = open_index(&state)?;
    let (indexed_path_id, _, _) = load_path_identity(&conn, &path_id)?;
    load_path_progress(&conn, &indexed_path_id).map_err(|err| err.to_string())
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
pub async fn check_source_drift(
    state: State<'_, AppState>,
    course_id: String,
) -> Result<SourceDriftStatus, String> {
    let conn = open_index(&state)?;
    let source = load_course_source_identity(&conn, &course_id)?;

    let checked_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let Some(origin_url) = source.origin_url.clone() else {
        return Ok(SourceDriftStatus {
            course_id: source.id,
            source_available: false,
            changed: false,
            current_hash: source.content_hash,
            latest_hash: None,
            origin_url: None,
            checked_at,
            orphaned_progress_paths: Vec::new(),
        });
    };

    let fetched = fetch_link(&origin_url)
        .await
        .map_err(|err| err.to_string())?;
    let latest_hash = fetched.source.content_hash.clone();
    let changed = source.content_hash.as_deref() != Some(latest_hash.as_str());
    let orphaned_progress_paths = if changed {
        vault::orphaned_progress_paths_for_markdown(
            &PathBuf::from(&source.vault_path),
            &fetched.content,
            fetched.title_hint.as_deref(),
        )
        .map_err(|err| err.to_string())?
    } else {
        Vec::new()
    };

    Ok(SourceDriftStatus {
        course_id: source.id,
        source_available: true,
        changed,
        current_hash: source.content_hash,
        latest_hash: Some(latest_hash),
        origin_url: Some(origin_url),
        checked_at,
        orphaned_progress_paths,
    })
}

#[tauri::command]
pub async fn reimport_course(
    state: State<'_, AppState>,
    course_id: String,
) -> Result<ReimportCourseResult, String> {
    let vault_path = state
        .vault_path
        .lock()
        .map_err(|_| "vault state lock poisoned".to_string())?
        .clone();

    let mut conn = open_index(&state)?;
    let source = load_course_source_identity(&conn, &course_id)?;
    let origin_url = source.origin_url.clone().ok_or_else(|| {
        "pasted courses cannot be re-imported because they have no source URL".to_string()
    })?;

    let fetched = fetch_link(&origin_url)
        .await
        .map_err(|err| err.to_string())?;
    let git_commit = git_vault::commit_all(
        &vault_path,
        &format!("Snapshot before re-importing {}", source.slug),
    )
    .map_err(|err| err.to_string())?;

    let reimported = vault::reimport_fetched_course(&vault_path, &source.slug, fetched)
        .map_err(|err| err.to_string())?;
    indexer::reindex_course(&mut conn, &vault_path, &source.slug).map_err(|err| err.to_string())?;
    let course = load_course_detail(&conn, &source.id)?;

    Ok(ReimportCourseResult {
        course,
        orphaned_progress_paths: reimported.orphaned_progress_paths,
        git_commit,
    })
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

fn load_course_source_identity(
    conn: &Connection,
    course_id: &str,
) -> Result<CourseSourceIdentity, String> {
    conn.query_row(
        "SELECT id, slug, vault_path, origin_url, content_hash
         FROM courses
         WHERE id = ?1 OR slug = ?1",
        params![course_id],
        |row| {
            Ok(CourseSourceIdentity {
                id: row.get(0)?,
                slug: row.get(1)?,
                vault_path: row.get(2)?,
                origin_url: row.get(3)?,
                content_hash: row.get(4)?,
            })
        },
    )
    .optional()
    .map_err(|err| err.to_string())?
    .ok_or_else(|| "course not found".to_string())
}

fn load_course_detail(conn: &Connection, course_id: &str) -> Result<CourseDetail, String> {
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

    let rows = load_section_rows(conn, &course.0)?;
    let sections = build_section_tree(&rows, None);
    let categories = load_course_categories(conn, &course.0).map_err(|err| err.to_string())?;
    let progress = load_course_progress(conn, &course.0).map_err(|err| err.to_string())?;

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

fn load_course_list_item(conn: &Connection, course_id: &str) -> Result<CourseListItem, String> {
    let (id, slug, title, description, section_count) = conn
        .query_row(
            "SELECT c.id, c.slug, c.title, c.description, COUNT(cs.id)
             FROM courses c
             LEFT JOIN course_sections cs ON cs.course_id = c.id
             WHERE c.id = ?1 OR c.slug = ?1
             GROUP BY c.id, c.slug, c.title, c.description",
            params![course_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i64>(4)? as usize,
                ))
            },
        )
        .optional()
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "course not found".to_string())?;

    Ok(CourseListItem {
        categories: load_course_categories(conn, &id).map_err(|err| err.to_string())?,
        progress: load_course_progress(conn, &id).map_err(|err| err.to_string())?,
        id,
        slug,
        title,
        description,
        section_count,
    })
}

fn load_path_identity(
    conn: &Connection,
    path_id: &str,
) -> Result<(String, String, String), String> {
    conn.query_row(
        "SELECT id, slug, vault_path FROM course_paths WHERE id = ?1 OR slug = ?1",
        params![path_id],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        },
    )
    .optional()
    .map_err(|err| err.to_string())?
    .ok_or_else(|| "path not found".to_string())
}

fn load_path_summary(conn: &Connection, path_id: &str) -> Result<CoursePathSummary, String> {
    let (id, slug, title, course_count) = conn
        .query_row(
            "SELECT cp.id, cp.slug, cp.title, COUNT(cpi.course_id)
             FROM course_paths cp
             LEFT JOIN course_path_items cpi ON cpi.course_path_id = cp.id
             WHERE cp.id = ?1 OR cp.slug = ?1
             GROUP BY cp.id, cp.slug, cp.title",
            params![path_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)? as usize,
                ))
            },
        )
        .optional()
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "path not found".to_string())?;

    Ok(CoursePathSummary {
        progress: load_path_progress(conn, &id).map_err(|err| err.to_string())?,
        id,
        slug,
        title,
        course_count,
    })
}

fn load_path_detail(conn: &Connection, path_id: &str) -> Result<CoursePathDetail, String> {
    let summary = load_path_summary(conn, path_id)?;
    let mut stmt = conn
        .prepare(
            "SELECT c.id, cpi.order_index, cpi.is_optional
             FROM course_path_items cpi
             JOIN courses c ON c.id = cpi.course_id
             WHERE cpi.course_path_id = ?1
             ORDER BY cpi.order_index",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map(params![summary.id.clone()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)? as usize,
                row.get::<_, i64>(2)? != 0,
            ))
        })
        .map_err(|err| err.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;

    let mut courses = Vec::new();
    for (course_id, order_index, optional) in rows {
        courses.push(CoursePathItem {
            course: load_course_list_item(conn, &course_id)?,
            order_index,
            optional,
        });
    }

    Ok(CoursePathDetail {
        id: summary.id,
        slug: summary.slug,
        title: summary.title,
        courses,
        progress: summary.progress,
    })
}

fn load_path_progress(conn: &Connection, path_id: &str) -> rusqlite::Result<CourseProgress> {
    let (total, not_started, in_progress, completed) = conn.query_row(
        "SELECT COUNT(cs.id),
                COALESCE(SUM(CASE WHEN COALESCE(sp.status, 'not_started') = 'not_started' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN sp.status = 'in_progress' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN sp.status = 'completed' THEN 1 ELSE 0 END), 0)
         FROM course_path_items cpi
         JOIN course_sections cs ON cs.course_id = cpi.course_id
         LEFT JOIN section_progress sp ON sp.section_id = cs.id
         WHERE cpi.course_path_id = ?1",
        params![path_id],
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

fn normalize_course_categories(
    vault_path: &std::path::Path,
    category_slugs: Vec<String>,
) -> anyhow::Result<Vec<String>> {
    let categories = read_categories_file(&vault_path.join("categories.yaml"))?;
    let existing = categories
        .iter()
        .map(|category| category.slug.as_str())
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();

    for slug in category_slugs {
        let slug = slug.trim();
        if slug.is_empty() || !seen.insert(slug.to_string()) {
            continue;
        }
        if !existing.contains(slug) {
            anyhow::bail!("unknown category: {slug}");
        }
        normalized.push(slug.to_string());
    }

    Ok(normalized)
}

fn read_categories_file(path: &std::path::Path) -> anyhow::Result<Vec<Category>> {
    if !path.is_file() {
        return Ok(Vec::new());
    }
    read_yaml_file(path)
}

fn write_categories_file(path: &std::path::Path, categories: &[Category]) -> anyhow::Result<()> {
    let mut categories = categories.to_vec();
    categories.sort_by_key(|category| category.name.to_lowercase());
    write_yaml_file(path, &categories)
}

fn read_path_manifest(path: &std::path::Path) -> Result<PathManifest, String> {
    read_yaml_file(path).map_err(|err| err.to_string())
}

fn unique_path_slug(vault_path: &std::path::Path, title: &str) -> String {
    let base = {
        let slug = slug::slugify(title);
        if slug.is_empty() {
            "path".to_string()
        } else {
            slug
        }
    };
    let paths_dir = vault_path.join("paths");
    let mut candidate = base.clone();
    let mut suffix = 2;

    while paths_dir.join(format!("{candidate}.yaml")).exists() {
        candidate = format!("{base}-{suffix}");
        suffix += 1;
    }

    candidate
}

fn unique_category_slug(name: &str, categories: &[Category]) -> String {
    let base = {
        let slug = slug::slugify(name);
        if slug.is_empty() {
            "category".to_string()
        } else {
            slug
        }
    };
    let existing = categories
        .iter()
        .map(|category| category.slug.as_str())
        .collect::<HashSet<_>>();
    let mut candidate = base.clone();
    let mut suffix = 2;

    while existing.contains(candidate.as_str()) {
        candidate = format!("{base}-{suffix}");
        suffix += 1;
    }

    candidate
}

fn read_yaml_file<T: for<'de> Deserialize<'de>>(path: &std::path::Path) -> anyhow::Result<T> {
    let contents = std::fs::read_to_string(path)?;
    Ok(serde_yaml::from_str(&contents)?)
}

fn write_yaml_file<T: Serialize>(path: &std::path::Path, value: &T) -> anyhow::Result<()> {
    let yaml = serde_yaml::to_string(value)?;
    std::fs::write(path, yaml)?;
    Ok(())
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
