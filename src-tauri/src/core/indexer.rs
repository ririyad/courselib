use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::core::models::{CourseManifest, SourceType};

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct ReindexSummary {
    pub courses: usize,
    pub sections: usize,
    pub categories: usize,
    pub paths: usize,
}

#[derive(Debug, Clone)]
struct SectionRecord {
    id: String,
    canonical_path: String,
    title: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct CategoryManifest {
    slug: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct PathManifest {
    title: String,
    slug: String,
    #[allow(dead_code)]
    description: Option<String>,
    courses: Vec<PathCourseManifest>,
}

#[derive(Debug, Deserialize)]
struct PathCourseManifest {
    slug: String,
    optional: bool,
}

#[derive(Debug, Deserialize, Default)]
struct ProgressEntry {
    #[serde(default = "not_started")]
    status: String,
    completed_at: Option<String>,
}

pub fn reindex_vault(conn: &mut Connection, vault_path: &Path) -> Result<ReindexSummary> {
    let tx = conn
        .transaction()
        .context("failed to start reindex transaction")?;
    clear_index(&tx)?;

    let categories = index_categories(&tx, vault_path)?;
    let mut summary = ReindexSummary {
        courses: 0,
        sections: 0,
        categories,
        paths: 0,
    };

    for course_dir in course_dirs(vault_path)? {
        let course_summary = index_course_in_tx(&tx, &course_dir)?;
        summary.courses += course_summary.courses;
        summary.sections += course_summary.sections;
    }

    summary.paths = index_paths(&tx, vault_path)?;
    tx.commit()
        .context("failed to commit reindex transaction")?;
    Ok(summary)
}

pub fn reindex_course(
    conn: &mut Connection,
    vault_path: &Path,
    course_slug: &str,
) -> Result<ReindexSummary> {
    let tx = conn
        .transaction()
        .context("failed to start course reindex transaction")?;

    index_categories(&tx, vault_path)?;
    delete_course_index(&tx, course_slug)?;
    let course_dir = vault_path.join("courses").join(course_slug);
    let mut summary = index_course_in_tx(&tx, &course_dir)?;
    summary.categories = count_rows(&tx, "categories")?;
    summary.paths = index_paths(&tx, vault_path)?;

    tx.commit()
        .context("failed to commit course reindex transaction")?;
    Ok(summary)
}

fn clear_index(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "DELETE FROM section_search;
         DELETE FROM section_progress;
         DELETE FROM course_path_items;
         DELETE FROM course_paths;
         DELETE FROM course_categories;
         DELETE FROM categories;
         DELETE FROM course_sections;
         DELETE FROM courses;",
    )
    .context("failed to clear SQLite index")?;
    Ok(())
}

fn delete_course_index(conn: &Connection, course_slug: &str) -> Result<()> {
    let course_id = course_id(course_slug);
    let mut stmt = conn
        .prepare("SELECT id FROM course_sections WHERE course_id = ?1")
        .context("failed to prepare existing section lookup")?;
    let rowids = stmt
        .query_map(params![course_id], |row| {
            let id: String = row.get(0)?;
            Ok(fts_rowid(&id))
        })
        .context("failed to read existing section IDs")?
        .collect::<Result<Vec<_>, _>>()?;

    for rowid in rowids {
        conn.execute(
            "DELETE FROM section_search WHERE rowid = ?1",
            params![rowid],
        )
        .context("failed to delete existing FTS row")?;
    }

    conn.execute("DELETE FROM courses WHERE slug = ?1", params![course_slug])
        .with_context(|| format!("failed to delete existing index for course {course_slug}"))?;
    Ok(())
}

fn index_course_in_tx(conn: &Connection, course_dir: &Path) -> Result<ReindexSummary> {
    let manifest_path = course_dir.join("_course.yaml");
    if !manifest_path.is_file() {
        return Ok(ReindexSummary {
            courses: 0,
            sections: 0,
            categories: 0,
            paths: 0,
        });
    }

    let manifest: CourseManifest = read_yaml(&manifest_path)?;
    let id = course_id(&manifest.slug);
    conn.execute(
        "INSERT INTO courses
         (id, slug, title, description, vault_path, source_type, origin_url, content_hash, imported_at, archived_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL)",
        params![
            id,
            manifest.slug,
            manifest.title,
            manifest.description,
            course_dir.to_string_lossy().as_ref(),
            source_type_str(manifest.source.source_type),
            manifest.source.origin_url,
            manifest.source.content_hash,
            manifest.source.imported_at,
        ],
    )
    .with_context(|| format!("failed to index course {}", manifest.slug))?;

    for category_slug in &manifest.categories {
        ensure_category(conn, category_slug, &titleize_slug(category_slug))?;
        conn.execute(
            "INSERT OR IGNORE INTO course_categories (course_id, category_id) VALUES (?1, ?2)",
            params![course_id(&manifest.slug), category_id(category_slug)],
        )
        .with_context(|| format!("failed to index category {category_slug}"))?;
    }

    let progress = read_progress(&course_dir.join("_progress.yaml"))?;
    let mut sections = Vec::new();
    let sections_dir = course_dir.join("sections");
    if sections_dir.is_dir() {
        index_sections_recursive(
            conn,
            &manifest.slug,
            &sections_dir,
            None,
            &[],
            &progress,
            &mut sections,
        )?;
    }

    for section in &sections {
        conn.execute(
            "INSERT INTO section_search(rowid, title, content) VALUES (?1, ?2, ?3)",
            params![fts_rowid(&section.id), section.title, section.content],
        )
        .with_context(|| format!("failed to index search for {}", section.canonical_path))?;
    }

    Ok(ReindexSummary {
        courses: 1,
        sections: sections.len(),
        categories: 0,
        paths: 0,
    })
}

fn index_sections_recursive(
    conn: &Connection,
    course_slug: &str,
    dir: &Path,
    parent_section_id: Option<String>,
    parent_parts: &[String],
    progress: &HashMap<String, ProgressEntry>,
    indexed: &mut Vec<SectionRecord>,
) -> Result<()> {
    for (order_index, entry_path) in ordered_section_entries(dir)?.into_iter().enumerate() {
        let file_name = entry_path
            .file_name()
            .and_then(|name| name.to_str())
            .context("section path has invalid file name")?;
        let slug = slug_from_prefixed_name(file_name);
        let mut canonical_parts = parent_parts.to_vec();
        canonical_parts.push(slug);
        let canonical_path = canonical_parts.join("/");
        let section_id = section_id(course_slug, &canonical_path);

        let body_path = if entry_path.is_dir() {
            entry_path.join("_index.md")
        } else {
            entry_path.clone()
        };
        let content = fs::read_to_string(&body_path)
            .with_context(|| format!("failed to read {}", body_path.display()))?;
        let metadata = markdown_section_metadata(&content, canonical_parts.last().unwrap());

        conn.execute(
            "INSERT INTO course_sections
             (id, course_id, parent_section_id, canonical_path, vault_path, title, heading_level, order_index)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                section_id,
                course_id(course_slug),
                parent_section_id,
                canonical_path,
                body_path.to_string_lossy().as_ref(),
                metadata.title,
                metadata.heading_level,
                order_index as i64,
            ],
        )
        .with_context(|| format!("failed to index section {canonical_path}"))?;

        let progress_entry = progress.get(&canonical_path);
        conn.execute(
            "INSERT INTO section_progress (section_id, status, completed_at) VALUES (?1, ?2, ?3)",
            params![
                section_id,
                progress_entry
                    .map(|entry| entry.status.as_str())
                    .unwrap_or("not_started"),
                progress_entry.and_then(|entry| entry.completed_at.as_deref()),
            ],
        )
        .with_context(|| format!("failed to index progress for {canonical_path}"))?;

        indexed.push(SectionRecord {
            id: section_id.clone(),
            canonical_path: canonical_path.clone(),
            title: metadata.title,
            content,
        });

        if entry_path.is_dir() {
            index_sections_recursive(
                conn,
                course_slug,
                &entry_path,
                Some(section_id),
                &canonical_parts,
                progress,
                indexed,
            )?;
        }
    }

    Ok(())
}

fn index_categories(conn: &Connection, vault_path: &Path) -> Result<usize> {
    let path = vault_path.join("categories.yaml");
    if !path.is_file() {
        return Ok(0);
    }

    let categories: Vec<CategoryManifest> = read_yaml(&path)?;
    for category in &categories {
        ensure_category(conn, &category.slug, &category.name)?;
    }
    Ok(categories.len())
}

fn index_paths(conn: &Connection, vault_path: &Path) -> Result<usize> {
    conn.execute("DELETE FROM course_path_items", [])
        .context("failed to clear path items")?;
    conn.execute("DELETE FROM course_paths", [])
        .context("failed to clear paths")?;

    let paths_dir = vault_path.join("paths");
    if !paths_dir.is_dir() {
        return Ok(0);
    }

    let mut count = 0;
    for entry in fs::read_dir(&paths_dir)
        .with_context(|| format!("failed to read {}", paths_dir.display()))?
    {
        let path = entry?.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("yaml") {
            continue;
        }

        let manifest: PathManifest = read_yaml(&path)?;
        let path_id = path_id(&manifest.slug);
        conn.execute(
            "INSERT INTO course_paths (id, slug, title, vault_path) VALUES (?1, ?2, ?3, ?4)",
            params![
                path_id,
                manifest.slug,
                manifest.title,
                path.to_string_lossy().as_ref()
            ],
        )
        .with_context(|| format!("failed to index path {}", manifest.slug))?;

        for (order_index, course) in manifest.courses.iter().enumerate() {
            if let Some(indexed_course_id) = conn
                .query_row(
                    "SELECT id FROM courses WHERE slug = ?1",
                    params![course.slug],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .context("failed to look up path course")?
            {
                conn.execute(
                    "INSERT INTO course_path_items
                     (course_path_id, course_id, order_index, is_optional)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![
                        path_id,
                        indexed_course_id,
                        order_index as i64,
                        if course.optional { 1 } else { 0 },
                    ],
                )
                .with_context(|| format!("failed to index path item {}", course.slug))?;
            }
        }
        count += 1;
    }

    Ok(count)
}

fn ensure_category(conn: &Connection, slug: &str, name: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO categories (id, slug, name) VALUES (?1, ?2, ?3)",
        params![category_id(slug), slug, name],
    )
    .with_context(|| format!("failed to index category {slug}"))?;
    Ok(())
}

fn course_dirs(vault_path: &Path) -> Result<Vec<PathBuf>> {
    let courses_dir = vault_path.join("courses");
    if !courses_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut dirs = fs::read_dir(&courses_dir)
        .with_context(|| format!("failed to read {}", courses_dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    dirs.sort();
    Ok(dirs)
}

fn ordered_section_entries(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut entries = fs::read_dir(dir)
        .with_context(|| format!("failed to read {}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if name.starts_with('_') || name.starts_with('.') {
                return false;
            }
            path.is_dir() || path.extension().and_then(|ext| ext.to_str()) == Some("md")
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(order_from_name)
            .unwrap_or(usize::MAX)
    });
    Ok(entries)
}

struct SectionMetadata {
    title: String,
    heading_level: u8,
}

fn markdown_section_metadata(content: &str, fallback_slug: &str) -> SectionMetadata {
    for line in content.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with('#') {
            continue;
        }
        let level = trimmed.chars().take_while(|char| *char == '#').count();
        if (1..=6).contains(&level) && trimmed.as_bytes().get(level) == Some(&b' ') {
            let title = trimmed[level..].trim().trim_matches('#').trim();
            if !title.is_empty() {
                return SectionMetadata {
                    title: title.to_string(),
                    heading_level: level as u8,
                };
            }
        }
    }

    SectionMetadata {
        title: titleize_slug(fallback_slug),
        heading_level: 1,
    }
}

fn read_progress(path: &Path) -> Result<HashMap<String, ProgressEntry>> {
    if !path.is_file() {
        return Ok(HashMap::new());
    }
    read_yaml(path)
}

fn read_yaml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_yaml::from_str(&contents).with_context(|| format!("failed to parse {}", path.display()))
}

fn slug_from_prefixed_name(name: &str) -> String {
    let without_ext = name.strip_suffix(".md").unwrap_or(name);
    if without_ext.len() > 3
        && without_ext.as_bytes()[0].is_ascii_digit()
        && without_ext.as_bytes()[1].is_ascii_digit()
        && without_ext.as_bytes()[2] == b'-'
    {
        without_ext[3..].to_string()
    } else {
        without_ext.to_string()
    }
}

fn order_from_name(name: &str) -> usize {
    name.get(0..2)
        .and_then(|prefix| prefix.parse::<usize>().ok())
        .unwrap_or(usize::MAX)
}

fn titleize_slug(slug: &str) -> String {
    slug.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn not_started() -> String {
    "not_started".to_string()
}

fn source_type_str(source_type: SourceType) -> &'static str {
    match source_type {
        SourceType::Github => "github",
        SourceType::Gitlab => "gitlab",
        SourceType::Codeberg => "codeberg",
        SourceType::Pasted => "pasted",
    }
}

fn course_id(slug: &str) -> String {
    format!("course:{slug}")
}

fn category_id(slug: &str) -> String {
    format!("category:{slug}")
}

fn section_id(course_slug: &str, canonical_path: &str) -> String {
    format!("section:{course_slug}:{canonical_path}")
}

fn path_id(slug: &str) -> String {
    format!("path:{slug}")
}

fn fts_rowid(id: &str) -> i64 {
    let digest = Sha256::digest(id.as_bytes());
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    (u64::from_be_bytes(bytes) & 0x7fff_ffff_ffff_ffff) as i64
}

fn count_rows(conn: &Connection, table: &str) -> Result<usize> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    let count: i64 = conn
        .query_row(&sql, [], |row| row.get(0))
        .with_context(|| format!("failed to count {table}"))?;
    Ok(count as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{source_fetch::fetched_from_paste, vault},
        db,
    };
    use rusqlite::Connection;
    use uuid::Uuid;

    #[test]
    fn full_reindex_populates_courses_sections_progress_and_search() {
        let vault_path = test_vault_path();
        let db_path = vault_path.join("index.sqlite");
        let markdown = "# Rust Course\nIntro\n\n## Setup\nInstall\n\n## Borrowing\nBorrow\n";
        let fetched = fetched_from_paste(markdown.to_string(), None);
        vault::write_fetched_course(&vault_path, fetched).expect("course should be written");
        fs::write(
            vault_path.join("courses/rust-course/_progress.yaml"),
            "rust-course/setup: {status: completed, completed_at: \"2026-07-10T08:00:00Z\"}\n",
        )
        .expect("progress should be written");

        let mut conn = open_test_db(&db_path);
        let summary = reindex_vault(&mut conn, &vault_path).expect("vault should reindex");

        assert_eq!(summary.courses, 1);
        assert_eq!(summary.sections, 3);
        assert_eq!(count(&conn, "courses"), 1);
        assert_eq!(count(&conn, "course_sections"), 3);
        assert_eq!(count(&conn, "section_search"), 3);
        let status: String = conn
            .query_row(
                "SELECT sp.status
                 FROM section_progress sp
                 JOIN course_sections cs ON cs.id = sp.section_id
                 WHERE cs.canonical_path = 'rust-course/setup'",
                [],
                |row| row.get(0),
            )
            .expect("status should be indexed");
        assert_eq!(status, "completed");

        fs::remove_dir_all(&vault_path).expect("test cleanup should succeed");
    }

    #[test]
    fn deleting_db_and_reindexing_reproduces_counts() {
        let vault_path = test_vault_path();
        let db_path = vault_path.join("index.sqlite");
        vault::write_fetched_course(
            &vault_path,
            fetched_from_paste("# One\n\n## Two\n".to_string(), None),
        )
        .expect("course should be written");

        let mut conn = open_test_db(&db_path);
        let first = reindex_vault(&mut conn, &vault_path).expect("first reindex");
        drop(conn);
        fs::remove_file(&db_path).expect("db should be deleted");

        let mut conn = open_test_db(&db_path);
        let second = reindex_vault(&mut conn, &vault_path).expect("second reindex");

        assert_eq!(first, second);
        assert_eq!(count(&conn, "courses"), 1);
        assert_eq!(count(&conn, "course_sections"), 2);

        fs::remove_dir_all(&vault_path).expect("test cleanup should succeed");
    }

    #[test]
    fn single_course_reindex_replaces_only_that_course() {
        let vault_path = test_vault_path();
        let db_path = vault_path.join("index.sqlite");
        vault::write_fetched_course(&vault_path, fetched_from_paste("# One\n".to_string(), None))
            .expect("first course should be written");
        vault::write_fetched_course(&vault_path, fetched_from_paste("# Two\n".to_string(), None))
            .expect("second course should be written");

        let mut conn = open_test_db(&db_path);
        reindex_vault(&mut conn, &vault_path).expect("full reindex");
        fs::write(
            vault_path.join("courses/one/sections/01-one.md"),
            "# One\n\nUpdated\n",
        )
        .expect("section should be updated");

        let summary = reindex_course(&mut conn, &vault_path, "one").expect("course reindex");

        assert_eq!(summary.courses, 1);
        assert_eq!(count(&conn, "courses"), 2);
        assert_eq!(count(&conn, "course_sections"), 2);

        fs::remove_dir_all(&vault_path).expect("test cleanup should succeed");
    }

    #[test]
    fn empty_vault_reindex_clears_courses() {
        let vault_path = test_vault_path();
        let db_path = vault_path.join("index.sqlite");
        vault::write_fetched_course(
            &vault_path,
            fetched_from_paste("# Temporary\n".to_string(), None),
        )
        .expect("course should be written");

        let mut conn = open_test_db(&db_path);
        reindex_vault(&mut conn, &vault_path).expect("initial reindex");
        assert_eq!(count(&conn, "courses"), 1);

        fs::remove_dir_all(vault_path.join("courses/temporary")).expect("course folder removed");
        let summary = reindex_vault(&mut conn, &vault_path).expect("empty reindex");

        assert_eq!(summary.courses, 0);
        assert_eq!(count(&conn, "courses"), 0);
        assert_eq!(count(&conn, "course_sections"), 0);

        fs::remove_dir_all(&vault_path).expect("test cleanup should succeed");
    }

    #[test]
    fn delete_course_then_reindex_clears_index_and_path_items() {
        let vault_path = test_vault_path();
        let db_path = vault_path.join("index.sqlite");
        let written = vault::write_fetched_course(
            &vault_path,
            fetched_from_paste("# Keep Path Clean\n".to_string(), None),
        )
        .expect("course should be written");
        fs::write(
            vault_path.join("paths/track.yaml"),
            format!(
                "title: Track\nslug: track\ndescription: null\ncourses:\n  - slug: {}\n    optional: false\n",
                written.slug
            ),
        )
        .expect("path should be written");

        let mut conn = open_test_db(&db_path);
        reindex_vault(&mut conn, &vault_path).expect("initial reindex");
        assert_eq!(count(&conn, "courses"), 1);
        assert_eq!(count(&conn, "course_path_items"), 1);

        vault::delete_course(&vault_path, &written.slug).expect("course deleted");
        let summary = reindex_vault(&mut conn, &vault_path).expect("reindex after delete");

        assert_eq!(summary.courses, 0);
        assert_eq!(count(&conn, "courses"), 0);
        assert_eq!(count(&conn, "course_path_items"), 0);
        assert_eq!(count(&conn, "course_paths"), 1);

        fs::remove_dir_all(&vault_path).expect("test cleanup should succeed");
    }

    fn open_test_db(db_path: &Path) -> Connection {
        let conn = db::open(db_path).expect("db should open");
        db::apply_schema(&conn).expect("schema should apply");
        conn
    }

    fn count(conn: &Connection, table: &str) -> usize {
        count_rows(conn, table).expect("count should work")
    }

    fn test_vault_path() -> PathBuf {
        std::env::temp_dir().join(format!("courselib-indexer-{}", Uuid::new_v4()))
    }
}
