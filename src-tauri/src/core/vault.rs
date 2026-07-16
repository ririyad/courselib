use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};

use crate::core::{
    git_vault,
    models::{AppSettings, AppStatus, CourseManifest, WrittenCourse, WrittenSection},
    parser::{parse_markdown_course, ParsedSection},
    source_fetch::FetchedMarkdown,
};

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

    fn test_vault_path() -> PathBuf {
        std::env::temp_dir().join(format!("courselib-vault-{}", Uuid::new_v4()))
    }
}
