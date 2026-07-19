use std::{fs, path::Path, thread, time::Duration};

use anyhow::{Context, Result};

const MAX_ATTEMPTS: u32 = 8;
const RETRY_DELAY_MS: u64 = 25;

/// Remove a directory tree, retrying briefly to tolerate Windows file locks.
pub fn remove_dir_all_retry(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    with_retry(path, "remove", || fs::remove_dir_all(path).map_err(|err| err.into()))
}

/// Rename a path, retrying briefly to tolerate Windows file locks.
pub fn rename_retry(from: &Path, to: &Path) -> Result<()> {
    with_retry(from, "rename", || {
        fs::rename(from, to).with_context(|| {
            format!(
                "failed to rename {} to {}",
                from.display(),
                to.display()
            )
        })
    })
}

fn with_retry<F>(path: &Path, action: &str, mut op: F) -> Result<()>
where
    F: FnMut() -> Result<()>,
{
    let mut last_error = None;

    for attempt in 1..=MAX_ATTEMPTS {
        match op() {
            Ok(()) => return Ok(()),
            Err(err) => {
                last_error = Some(err);
                if attempt < MAX_ATTEMPTS {
                    thread::sleep(Duration::from_millis(RETRY_DELAY_MS * u64::from(attempt)));
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        anyhow::anyhow!("failed to {action} {} after {MAX_ATTEMPTS} attempts", path.display())
    }))
    .with_context(|| format!("failed to {action} {}", path.display()))
}
