use std::{fs, path::Path};

use anyhow::{bail, Context, Result};
use git2::Repository;

/// Ensure the vault has Git metadata stored in `vault/.vaultgit`.
///
/// A small `vault/.git` gitdir pointer is written so normal Git tooling and
/// git2 can open the vault as a work tree while keeping the actual metadata in
/// the app-owned `.vaultgit` folder required by the vault spec.
pub fn ensure_initialized(vault_path: &Path) -> Result<()> {
    let git_link_path = vault_path.join(".git");
    let vault_git_path = vault_path.join(".vaultgit");

    if vault_git_path.is_dir() {
        if !git_link_path.exists() {
            fs::write(&git_link_path, "gitdir: .vaultgit\n")
                .with_context(|| format!("failed to write {}", git_link_path.display()))?;
        }
        return Ok(());
    }

    if git_link_path.exists() {
        bail!(
            "{} already contains Git metadata; choose a vault folder without an existing .git entry",
            vault_path.display()
        );
    }

    Repository::init(vault_path)
        .with_context(|| format!("failed to initialize git repo in {}", vault_path.display()))?;

    let default_git_path = vault_path.join(".git");
    if default_git_path.is_dir() {
        fs::rename(&default_git_path, &vault_git_path).with_context(|| {
            format!(
                "failed to move {} to {}",
                default_git_path.display(),
                vault_git_path.display()
            )
        })?;
        fs::write(&git_link_path, "gitdir: .vaultgit\n")
            .with_context(|| format!("failed to write {}", git_link_path.display()))?;
    }

    Ok(())
}
