use std::{fs, path::Path};

use anyhow::{bail, Context, Result};
use git2::{IndexAddOption, Oid, Repository, Signature, StatusOptions};

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

pub fn commit_all(vault_path: &Path, message: &str) -> Result<Option<String>> {
    ensure_initialized(vault_path)?;

    let repo = Repository::open(vault_path)
        .with_context(|| format!("failed to open git repo in {}", vault_path.display()))?;
    let mut index = repo.index().context("failed to open git index")?;
    index
        .add_all(
            ["courses", "paths", "categories.yaml"].iter(),
            IndexAddOption::DEFAULT,
            None,
        )
        .context("failed to stage vault files")?;
    index.write().context("failed to write git index")?;

    if head_exists(&repo) && worktree_clean(&repo)? {
        return Ok(None);
    }

    let tree_oid = index.write_tree().context("failed to write git tree")?;
    let tree = repo
        .find_tree(tree_oid)
        .context("failed to find git tree")?;
    let signature = Signature::now("CourseLib", "courselib@local")
        .context("failed to create git commit signature")?;

    let commit_oid: Oid = match repo.head().and_then(|head| head.peel_to_commit()) {
        Ok(parent) => repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &[&parent],
            )
            .context("failed to commit vault snapshot")?,
        Err(_) => repo
            .commit(Some("HEAD"), &signature, &signature, message, &tree, &[])
            .context("failed to create initial vault snapshot")?,
    };

    Ok(Some(commit_oid.to_string()))
}

fn head_exists(repo: &Repository) -> bool {
    repo.head().and_then(|head| head.peel_to_commit()).is_ok()
}

fn worktree_clean(repo: &Repository) -> Result<bool> {
    let mut options = StatusOptions::new();
    options.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = repo
        .statuses(Some(&mut options))
        .context("failed to read git status")?;

    Ok(statuses.iter().all(|entry| {
        let path = entry.path().unwrap_or_default();
        path.starts_with(".vaultgit") || path == ".git"
    }))
}
