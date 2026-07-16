use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};

use crate::core::{
    git_vault,
    models::{AppSettings, AppStatus},
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

    fn test_vault_path() -> PathBuf {
        std::env::temp_dir().join(format!("courselib-vault-{}", Uuid::new_v4()))
    }
}
