use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};

use crate::core::{
    git_vault,
    models::{
        AppSettings, AppStatus, Category, CourseManifest, SectionProgressEntry, WrittenCourse,
        WrittenSection,
    },
    parser::{parse_markdown_course, ParsedSection},
    source_fetch::FetchedMarkdown,
};
use serde::{Deserialize, Serialize};

pub fn load_or_default_vault_path(app: &AppHandle) -> Result<PathBuf> {
    let settings_path = settings_path(app)?;
    if settings_path.exists() {
        let contents = fs::read_to_string(&settings_path)
            .with_context(|| format!("failed to read {}", settings_path.display()))?;
        let settings: AppSettings = serde_yaml::from_str(&contents)
            .with_context(|| format!("failed to parse {}", settings_path.display()))?;
        return Ok(PathBuf::from(settings.vault_path));
    }

    Ok(default_vault_path())
}

pub fn save_vault_path(app: &AppHandle, vault_path: &Path) -> Result<()> {
    let settings_path = settings_path(app)?;
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let settings = AppSettings {
        vault_path: vault_path.to_string_lossy().into_owned(),
    };
    let contents = serde_yaml::to_string(&settings).context("failed to serialize settings")?;
    fs::write(&settings_path, contents)
        .with_context(|| format!("failed to write {}", settings_path.display()))?;

    Ok(())
}

pub fn ensure_vault(vault_path: &Path) -> Result<()> {
    fs::create_dir_all(vault_path)
        .with_context(|| format!("failed to create {}", vault_path.display()))?;
    fs::create_dir_all(vault_path.join("courses")).with_context(|| {
        format!(
            "failed to create courses folder in {}",
            vault_path.display()
        )
    })?;
    fs::create_dir_all(vault_path.join("paths"))
        .with_context(|| format!("failed to create paths folder in {}", vault_path.display()))?;

    let categories_path = vault_path.join("categories.yaml");
    if !categories_path.exists() {
        fs::write(&categories_path, "[]\n")
            .with_context(|| format!("failed to write {}", categories_path.display()))?;
    }

    git_vault::ensure_initialized(vault_path)?;

    Ok(())
}

pub fn status(vault_path: &Path) -> AppStatus {
    AppStatus {
        vault_path: vault_path.to_string_lossy().into_owned(),
        courses_dir_exists: vault_path.join("courses").is_dir(),
        paths_dir_exists: vault_path.join("paths").is_dir(),
        categories_file_exists: vault_path.join("categories.yaml").is_file(),
        vault_git_initialized: vault_path.join(".vaultgit").is_dir(),
    }
}

pub fn write_fetched_course(vault_path: &Path, fetched: FetchedMarkdown) -> Result<WrittenCourse> {
    ensure_vault(vault_path)?;

    let parsed = parse_markdown_course(&fetched.content, fetched.title_hint.as_deref());
    let course_slug = unique_course_slug(vault_path, &parsed.title);
    let course_path = vault_path.join("courses").join(&course_slug);
    let sections_path = course_path.join("sections");

    fs::create_dir_all(&sections_path)
        .with_context(|| format!("failed to create {}", sections_path.display()))?;

    fs::write(course_path.join("_source.md"), &fetched.content)
        .with_context(|| format!("failed to write source snapshot for {course_slug}"))?;

    let manifest = CourseManifest {
        title: parsed.title.clone(),
        slug: course_slug.clone(),
        description: None,
        categories: Vec::new(),
        source: fetched.source,
    };
    let manifest_yaml =
        serde_yaml::to_string(&manifest).context("failed to serialize course manifest")?;
    fs::write(course_path.join("_course.yaml"), manifest_yaml)
        .with_context(|| format!("failed to write _course.yaml for {course_slug}"))?;

    fs::write(course_path.join("_progress.yaml"), "{}\n")
        .with_context(|| format!("failed to write _progress.yaml for {course_slug}"))?;

    let sections = write_sections(&sections_path, &parsed.sections, &[])?;

    Ok(WrittenCourse {
        title: parsed.title,
        slug: course_slug,
        vault_path: course_path.to_string_lossy().into_owned(),
        sections,
    })
}

pub fn delete_course(vault_path: &Path, course_slug: &str) -> Result<()> {
    ensure_vault(vault_path)?;

    let course_path = vault_path.join("courses").join(course_slug);
    if !course_path.is_dir() {
        anyhow::bail!("course `{course_slug}` not found in vault");
    }

    fs::remove_dir_all(&course_path)
        .with_context(|| format!("failed to delete course folder {}", course_path.display()))?;

    remove_course_from_paths(vault_path, course_slug)?;
    Ok(())
}

pub fn rename_category(vault_path: &Path, category_slug: &str, name: &str) -> Result<Category> {
    ensure_vault(vault_path)?;

    let name = name.trim();
    if name.is_empty() {
        anyhow::bail!("category name cannot be empty");
    }

    let categories_path = vault_path.join("categories.yaml");
    let mut categories = read_categories_file(&categories_path)?;
    let category_index = categories
        .iter()
        .position(|category| category.slug == category_slug)
        .ok_or_else(|| anyhow::anyhow!("category `{category_slug}` not found"))?;
    let new_slug = base_slug(name, "category");

    if new_slug != category_slug && categories.iter().any(|category| category.slug == new_slug) {
        anyhow::bail!("category `{new_slug}` already exists");
    }

    let renamed = Category {
        slug: new_slug.clone(),
        name: name.to_string(),
    };
    categories[category_index] = renamed.clone();
    write_categories_file(&categories_path, &categories)?;

    if new_slug != category_slug {
        update_course_category_references(vault_path, category_slug, Some(&new_slug))?;
    }

    Ok(renamed)
}

pub fn delete_category(vault_path: &Path, category_slug: &str) -> Result<usize> {
    ensure_vault(vault_path)?;

    let categories_path = vault_path.join("categories.yaml");
    let mut categories = read_categories_file(&categories_path)?;
    let before = categories.len();
    categories.retain(|category| category.slug != category_slug);
    if categories.len() == before {
        anyhow::bail!("category `{category_slug}` not found");
    }

    write_categories_file(&categories_path, &categories)?;
    update_course_category_references(vault_path, category_slug, None)
}

fn remove_course_from_paths(vault_path: &Path, course_slug: &str) -> Result<()> {
    let paths_dir = vault_path.join("paths");
    if !paths_dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(&paths_dir)
        .with_context(|| format!("failed to read {}", paths_dir.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", paths_dir.display()))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("yaml") {
            continue;
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let mut manifest: PathManifest = serde_yaml::from_str(&contents)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        let before = manifest.courses.len();
        manifest.courses.retain(|course| course.slug != course_slug);
        if manifest.courses.len() == before {
            continue;
        }

        let yaml = serde_yaml::to_string(&manifest)
            .with_context(|| format!("failed to serialize {}", path.display()))?;
        fs::write(&path, yaml).with_context(|| format!("failed to write {}", path.display()))?;
    }

    Ok(())
}

#[derive(Debug, Deserialize, Serialize)]
struct PathManifest {
    title: String,
    slug: String,
    description: Option<String>,
    courses: Vec<PathCourseManifest>,
}

#[derive(Debug, Deserialize, Serialize)]
struct PathCourseManifest {
    slug: String,
    optional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReimportedCourse {
    pub written: WrittenCourse,
    pub orphaned_progress_paths: Vec<String>,
}

pub fn reimport_fetched_course(
    vault_path: &Path,
    course_slug: &str,
    fetched: FetchedMarkdown,
) -> Result<ReimportedCourse> {
    ensure_vault(vault_path)?;

    let course_path = vault_path.join("courses").join(course_slug);
    let manifest_path = course_path.join("_course.yaml");
    let progress_path = course_path.join("_progress.yaml");
    let sections_path = course_path.join("sections");
    let temp_sections_path = course_path.join("sections.__reimport");

    if !manifest_path.is_file() {
        anyhow::bail!("course manifest not found for {course_slug}");
    }

    let mut manifest: CourseManifest = read_yaml_file(&manifest_path)?;
    let parsed = parse_markdown_course(&fetched.content, fetched.title_hint.as_deref());

    if temp_sections_path.exists() {
        fs::remove_dir_all(&temp_sections_path)
            .with_context(|| format!("failed to remove {}", temp_sections_path.display()))?;
    }
    fs::create_dir_all(&temp_sections_path)
        .with_context(|| format!("failed to create {}", temp_sections_path.display()))?;

    let sections = write_sections(&temp_sections_path, &parsed.sections, &[])?;
    let canonical_paths = written_canonical_paths(&sections);
    let orphaned_progress_paths = prune_progress_file(&progress_path, &canonical_paths)?;

    fs::write(course_path.join("_source.md"), &fetched.content)
        .with_context(|| format!("failed to write source snapshot for {course_slug}"))?;

    manifest.source = fetched.source;
    let manifest_yaml =
        serde_yaml::to_string(&manifest).context("failed to serialize course manifest")?;
    fs::write(&manifest_path, manifest_yaml)
        .with_context(|| format!("failed to write {}", manifest_path.display()))?;

    if sections_path.exists() {
        fs::remove_dir_all(&sections_path)
            .with_context(|| format!("failed to remove {}", sections_path.display()))?;
    }
    fs::rename(&temp_sections_path, &sections_path).with_context(|| {
        format!(
            "failed to replace {} with {}",
            sections_path.display(),
            temp_sections_path.display()
        )
    })?;

    Ok(ReimportedCourse {
        written: WrittenCourse {
            title: manifest.title,
            slug: manifest.slug,
            vault_path: course_path.to_string_lossy().into_owned(),
            sections,
        },
        orphaned_progress_paths,
    })
}

pub fn canonical_paths_for_markdown(markdown: &str, title_hint: Option<&str>) -> HashSet<String> {
    let parsed = parse_markdown_course(markdown, title_hint);
    let mut paths = HashSet::new();
    collect_section_canonical_paths(&parsed.sections, &[], &mut paths);
    paths
}

pub fn orphaned_progress_paths_for_markdown(
    course_path: &Path,
    markdown: &str,
    title_hint: Option<&str>,
) -> Result<Vec<String>> {
    let canonical_paths = canonical_paths_for_markdown(markdown, title_hint);
    let progress = read_progress_file(&course_path.join("_progress.yaml"))?;
    let mut orphaned = progress
        .keys()
        .filter(|path| !canonical_paths.contains(*path))
        .cloned()
        .collect::<Vec<_>>();
    orphaned.sort();
    Ok(orphaned)
}

fn write_sections(
    base_path: &Path,
    sections: &[ParsedSection],
    parent_slugs: &[String],
) -> Result<Vec<WrittenSection>> {
    let mut used_slugs = HashSet::new();
    let mut written = Vec::with_capacity(sections.len());

    for (index, section) in sections.iter().enumerate() {
        let section_slug = unique_sibling_slug(&section.title, &mut used_slugs);
        let prefixed_name = format!("{:02}-{}", index + 1, section_slug);
        let mut canonical_parts = parent_slugs.to_vec();
        canonical_parts.push(section_slug);
        let canonical_path = canonical_parts.join("/");

        if section.children.is_empty() {
            let file_path = base_path.join(format!("{prefixed_name}.md"));
            fs::write(&file_path, &section.body)
                .with_context(|| format!("failed to write {}", file_path.display()))?;
            written.push(WrittenSection {
                title: section.title.clone(),
                canonical_path,
                vault_path: file_path.to_string_lossy().into_owned(),
                heading_level: section.level,
                order_index: index,
                children: Vec::new(),
            });
        } else {
            let dir_path = base_path.join(&prefixed_name);
            fs::create_dir_all(&dir_path)
                .with_context(|| format!("failed to create {}", dir_path.display()))?;
            let index_path = dir_path.join("_index.md");
            fs::write(&index_path, &section.body)
                .with_context(|| format!("failed to write {}", index_path.display()))?;
            let children = write_sections(&dir_path, &section.children, &canonical_parts)?;
            written.push(WrittenSection {
                title: section.title.clone(),
                canonical_path,
                vault_path: index_path.to_string_lossy().into_owned(),
                heading_level: section.level,
                order_index: index,
                children,
            });
        }
    }

    Ok(written)
}

fn collect_section_canonical_paths(
    sections: &[ParsedSection],
    parent_slugs: &[String],
    paths: &mut HashSet<String>,
) {
    let mut used_slugs = HashSet::new();

    for section in sections {
        let section_slug = unique_sibling_slug(&section.title, &mut used_slugs);
        let mut canonical_parts = parent_slugs.to_vec();
        canonical_parts.push(section_slug);
        paths.insert(canonical_parts.join("/"));
        collect_section_canonical_paths(&section.children, &canonical_parts, paths);
    }
}

fn written_canonical_paths(sections: &[WrittenSection]) -> HashSet<String> {
    let mut paths = HashSet::new();
    collect_written_canonical_paths(sections, &mut paths);
    paths
}

fn collect_written_canonical_paths(sections: &[WrittenSection], paths: &mut HashSet<String>) {
    for section in sections {
        paths.insert(section.canonical_path.clone());
        collect_written_canonical_paths(&section.children, paths);
    }
}

fn read_progress_file(
    path: &Path,
) -> Result<std::collections::BTreeMap<String, SectionProgressEntry>> {
    if !path.is_file() {
        return Ok(std::collections::BTreeMap::new());
    }

    let contents =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_yaml::from_str(&contents).with_context(|| format!("failed to parse {}", path.display()))
}

fn read_categories_file(path: &Path) -> Result<Vec<Category>> {
    if !path.is_file() {
        return Ok(Vec::new());
    }
    read_yaml_file(path)
}

fn write_categories_file(path: &Path, categories: &[Category]) -> Result<()> {
    let mut categories = categories.to_vec();
    categories.sort_by_key(|category| category.name.to_lowercase());
    write_yaml_file(path, &categories)
}

fn update_course_category_references(
    vault_path: &Path,
    old_slug: &str,
    replacement_slug: Option<&str>,
) -> Result<usize> {
    let courses_dir = vault_path.join("courses");
    if !courses_dir.is_dir() {
        return Ok(0);
    }

    let mut changed_courses = 0;
    for entry in fs::read_dir(&courses_dir)
        .with_context(|| format!("failed to read {}", courses_dir.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", courses_dir.display()))?;
        let course_path = entry.path();
        if !course_path.is_dir() {
            continue;
        }

        let manifest_path = course_path.join("_course.yaml");
        if !manifest_path.is_file() {
            continue;
        }

        let mut manifest: CourseManifest = read_yaml_file(&manifest_path)?;
        let mut changed = false;
        let mut seen = HashSet::new();
        let mut next_categories = Vec::new();

        for category in &manifest.categories {
            let next = if category == old_slug {
                changed = true;
                replacement_slug
            } else {
                Some(category.as_str())
            };

            if let Some(next) = next {
                if seen.insert(next.to_string()) {
                    next_categories.push(next.to_string());
                } else {
                    changed = true;
                }
            }
        }

        if changed {
            manifest.categories = next_categories;
            write_yaml_file(&manifest_path, &manifest)?;
            changed_courses += 1;
        }
    }

    Ok(changed_courses)
}

fn prune_progress_file(path: &Path, canonical_paths: &HashSet<String>) -> Result<Vec<String>> {
    let mut progress = read_progress_file(path)?;
    let mut orphaned = progress
        .keys()
        .filter(|progress_path| !canonical_paths.contains(*progress_path))
        .cloned()
        .collect::<Vec<_>>();
    orphaned.sort();

    if !orphaned.is_empty() {
        progress.retain(|progress_path, _| canonical_paths.contains(progress_path));
        let yaml = if progress.is_empty() {
            "{}\n".to_string()
        } else {
            serde_yaml::to_string(&progress).context("failed to serialize progress")?
        };
        fs::write(path, yaml).with_context(|| format!("failed to write {}", path.display()))?;
    }

    Ok(orphaned)
}

fn read_yaml_file<T: for<'de> serde::Deserialize<'de>>(path: &Path) -> Result<T> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_yaml::from_str(&contents).with_context(|| format!("failed to parse {}", path.display()))
}

fn write_yaml_file<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    let yaml = serde_yaml::to_string(value)
        .with_context(|| format!("failed to serialize {}", path.display()))?;
    fs::write(path, yaml).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn unique_course_slug(vault_path: &Path, title: &str) -> String {
    let courses_path = vault_path.join("courses");
    let base = base_slug(title, "course");
    let mut candidate = base.clone();
    let mut suffix = 2;

    while courses_path.join(&candidate).exists() {
        candidate = format!("{base}-{suffix}");
        suffix += 1;
    }

    candidate
}

fn unique_sibling_slug(title: &str, used: &mut HashSet<String>) -> String {
    let base = base_slug(title, "section");
    let mut candidate = base.clone();
    let mut suffix = 2;

    while used.contains(&candidate) {
        candidate = format!("{base}-{suffix}");
        suffix += 1;
    }

    used.insert(candidate.clone());
    candidate
}

fn base_slug(title: &str, fallback: &str) -> String {
    let slug = slug::slugify(title);
    if slug.is_empty() {
        fallback.to_string()
    } else {
        slug
    }
}

fn default_vault_path() -> PathBuf {
    dirs::document_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join("CourseLib Vault")
}

fn settings_path(app: &AppHandle) -> Result<PathBuf> {
    Ok(app
        .path()
        .app_config_dir()
        .context("failed to resolve app config directory")?
        .join("settings.yaml"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::source_fetch::fetched_from_paste;
    use git2::Repository;
    use uuid::Uuid;

    #[test]
    fn ensure_vault_creates_required_layout_and_git_metadata() {
        let vault_path = test_vault_path();
        let _ = fs::remove_dir_all(&vault_path);

        ensure_vault(&vault_path).expect("vault should be initialized");

        assert!(vault_path.join("courses").is_dir());
        assert!(vault_path.join("paths").is_dir());
        assert!(vault_path.join("categories.yaml").is_file());
        assert!(vault_path.join(".vaultgit").is_dir());
        assert!(Repository::open(&vault_path).is_ok());

        fs::remove_dir_all(&vault_path).expect("test vault cleanup should succeed");
    }

    #[test]
    fn ensure_vault_does_not_rewrite_existing_git_repo() {
        let vault_path = test_vault_path();
        let _ = fs::remove_dir_all(&vault_path);
        fs::create_dir_all(vault_path.join(".git")).expect("test git folder should be created");

        let err = ensure_vault(&vault_path).expect_err("existing .git should be rejected");

        assert!(err.to_string().contains("already contains Git metadata"));
        assert!(vault_path.join(".git").is_dir());
        assert!(!vault_path.join(".vaultgit").exists());

        fs::remove_dir_all(&vault_path).expect("test vault cleanup should succeed");
    }

    #[test]
    fn write_fetched_course_creates_spec_layout() {
        let vault_path = test_vault_path();
        let _ = fs::remove_dir_all(&vault_path);
        let markdown = "# Rust Course\nIntro\n\n## Setup\nInstall Rust\n\n## Setup\nCollision\n\n### Cargo\nUse cargo\n";
        let fetched = fetched_from_paste(markdown.to_string(), None);

        let written = write_fetched_course(&vault_path, fetched).expect("course should be written");

        let course_path = vault_path.join("courses/rust-course");
        assert_eq!(written.slug, "rust-course");
        assert!(course_path.join("_source.md").is_file());
        assert!(course_path.join("_course.yaml").is_file());
        assert!(course_path.join("_progress.yaml").is_file());
        assert!(course_path
            .join("sections/01-rust-course/_index.md")
            .is_file());
        assert!(course_path
            .join("sections/01-rust-course/01-setup.md")
            .is_file());
        assert!(course_path
            .join("sections/01-rust-course/02-setup-2/_index.md")
            .is_file());
        assert!(course_path
            .join("sections/01-rust-course/02-setup-2/01-cargo.md")
            .is_file());
        assert_eq!(
            written.sections[0].children[1].canonical_path,
            "rust-course/setup-2"
        );

        fs::remove_dir_all(&vault_path).expect("test vault cleanup should succeed");
    }

    #[test]
    fn write_fetched_course_uses_unique_course_slug() {
        let vault_path = test_vault_path();
        let _ = fs::remove_dir_all(&vault_path);

        let first = fetched_from_paste("# Same\n".to_string(), None);
        let second = fetched_from_paste("# Same\n".to_string(), None);

        let first_written = write_fetched_course(&vault_path, first).expect("first course");
        let second_written = write_fetched_course(&vault_path, second).expect("second course");

        assert_eq!(first_written.slug, "same");
        assert_eq!(second_written.slug, "same-2");
        assert!(vault_path.join("courses/same").is_dir());
        assert!(vault_path.join("courses/same-2").is_dir());

        fs::remove_dir_all(&vault_path).expect("test vault cleanup should succeed");
    }

    #[test]
    fn delete_course_removes_folder_and_path_membership() {
        let vault_path = test_vault_path();
        let _ = fs::remove_dir_all(&vault_path);

        let written = write_fetched_course(
            &vault_path,
            fetched_from_paste("# Delete Me\n\n## One\n".to_string(), None),
        )
        .expect("course should be written");

        let path_yaml = vault_path.join("paths/learning.yaml");
        fs::write(
            &path_yaml,
            format!(
                "title: Learning\nslug: learning\ndescription: null\ncourses:\n  - slug: {}\n    optional: false\n  - slug: other\n    optional: true\n",
                written.slug
            ),
        )
        .expect("path should be written");

        delete_course(&vault_path, &written.slug).expect("course should be deleted");

        assert!(!vault_path.join("courses").join(&written.slug).exists());
        let path_contents = fs::read_to_string(&path_yaml).expect("path should remain");
        assert!(!path_contents.contains(&written.slug));
        assert!(path_contents.contains("other"));

        let missing = delete_course(&vault_path, &written.slug).expect_err("missing course");
        assert!(missing.to_string().contains("not found"));

        fs::remove_dir_all(&vault_path).expect("test vault cleanup should succeed");
    }

    #[test]
    fn rename_category_updates_catalog_and_course_manifests() {
        let vault_path = test_vault_path();
        let _ = fs::remove_dir_all(&vault_path);

        let written = write_fetched_course(
            &vault_path,
            fetched_from_paste("# Systems\n\n## One\n".to_string(), None),
        )
        .expect("course should be written");
        fs::write(
            vault_path.join("categories.yaml"),
            "- slug: distributed-sytems\n  name: Distributed Sytems\n- slug: python\n  name: Python\n",
        )
        .expect("categories should be written");

        let manifest_path = vault_path
            .join("courses")
            .join(&written.slug)
            .join("_course.yaml");
        let mut manifest: CourseManifest = read_yaml_file(&manifest_path).expect("manifest");
        manifest.categories = vec!["distributed-sytems".to_string(), "python".to_string()];
        write_yaml_file(&manifest_path, &manifest).expect("manifest should be updated");

        let renamed = rename_category(&vault_path, "distributed-sytems", "Distributed Systems")
            .expect("category should be renamed");

        assert_eq!(renamed.slug, "distributed-systems");
        assert_eq!(renamed.name, "Distributed Systems");
        let categories =
            read_categories_file(&vault_path.join("categories.yaml")).expect("categories");
        assert!(categories
            .iter()
            .any(|category| category.slug == "distributed-systems"));
        assert!(!categories
            .iter()
            .any(|category| category.slug == "distributed-sytems"));

        let manifest: CourseManifest = read_yaml_file(&manifest_path).expect("manifest");
        assert_eq!(
            manifest.categories,
            vec!["distributed-systems".to_string(), "python".to_string()]
        );

        let collision = rename_category(&vault_path, "distributed-systems", "Python")
            .expect_err("duplicate slug should be rejected");
        assert!(collision.to_string().contains("already exists"));

        fs::remove_dir_all(&vault_path).expect("test vault cleanup should succeed");
    }

    #[test]
    fn delete_category_removes_catalog_entry_and_course_references() {
        let vault_path = test_vault_path();
        let _ = fs::remove_dir_all(&vault_path);

        let written = write_fetched_course(
            &vault_path,
            fetched_from_paste("# Math\n\n## One\n".to_string(), None),
        )
        .expect("course should be written");
        fs::write(
            vault_path.join("categories.yaml"),
            "- slug: discrete-math\n  name: Discrete Math\n- slug: python\n  name: Python\n",
        )
        .expect("categories should be written");

        let manifest_path = vault_path
            .join("courses")
            .join(&written.slug)
            .join("_course.yaml");
        let mut manifest: CourseManifest = read_yaml_file(&manifest_path).expect("manifest");
        manifest.categories = vec!["discrete-math".to_string(), "python".to_string()];
        write_yaml_file(&manifest_path, &manifest).expect("manifest should be updated");

        let removed_from_courses =
            delete_category(&vault_path, "discrete-math").expect("category should be deleted");

        assert_eq!(removed_from_courses, 1);
        let categories =
            read_categories_file(&vault_path.join("categories.yaml")).expect("categories");
        assert!(!categories
            .iter()
            .any(|category| category.slug == "discrete-math"));
        assert!(categories.iter().any(|category| category.slug == "python"));

        let manifest: CourseManifest = read_yaml_file(&manifest_path).expect("manifest");
        assert_eq!(manifest.categories, vec!["python".to_string()]);

        let missing = delete_category(&vault_path, "discrete-math").expect_err("missing category");
        assert!(missing.to_string().contains("not found"));

        fs::remove_dir_all(&vault_path).expect("test vault cleanup should succeed");
    }

    fn test_vault_path() -> PathBuf {
        std::env::temp_dir().join(format!("courselib-vault-{}", Uuid::new_v4()))
    }
}
