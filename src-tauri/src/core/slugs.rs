use sha2::{Digest, Sha256};

/// Keep on-disk path components comfortably under Windows MAX_PATH pressure
/// when nested under vault/courses/.../sections/...
const MAX_SLUG_LEN: usize = 60;

/// Build a filesystem-safe slug from a human title.
pub fn base_slug(title: &str, fallback: &str) -> String {
    let raw = slug::slugify(title);
    let slug = if raw.is_empty() {
        fallback
    } else {
        raw.as_str()
    };
    sanitize_slug(slug, fallback)
}

/// Ensure a slug is safe as a Windows/macOS path component.
pub fn sanitize_slug(slug: &str, fallback: &str) -> String {
    let mut candidate = slug.trim().to_ascii_lowercase();
    if candidate.is_empty() {
        candidate = fallback.to_ascii_lowercase();
    }

    candidate = truncate_with_hash(&candidate, MAX_SLUG_LEN);

    if is_reserved_windows_name(&candidate) {
        candidate = format!("{candidate}-item");
        candidate = truncate_with_hash(&candidate, MAX_SLUG_LEN);
    }

    if candidate.is_empty() {
        fallback.to_ascii_lowercase()
    } else {
        candidate
    }
}

/// Produce a unique slug by appending numeric suffixes while staying within length limits.
pub fn unique_slug<F>(base: &str, mut is_taken: F) -> String
where
    F: FnMut(&str) -> bool,
{
    let safe_base = sanitize_slug(base, "item");
    let mut candidate = safe_base.clone();
    let mut suffix = 2u32;

    while is_taken(&candidate) {
        candidate = with_numeric_suffix(&safe_base, suffix);
        suffix = suffix.saturating_add(1);
    }

    candidate
}

fn with_numeric_suffix(base: &str, suffix: u32) -> String {
    let suffix_text = format!("-{suffix}");
    let max_base_len = MAX_SLUG_LEN.saturating_sub(suffix_text.len());
    let truncated = truncate_with_hash(base, max_base_len.max(1));
    format!("{truncated}{suffix_text}")
}

fn truncate_with_hash(value: &str, max_len: usize) -> String {
    if max_len == 0 {
        return String::new();
    }
    if value.len() <= max_len {
        return value.to_string();
    }

    let hash = short_hash(value);
    if hash.len() + 1 >= max_len {
        return hash.chars().take(max_len).collect();
    }

    let keep = max_len - hash.len() - 1;
    let prefix = truncate_bytes(value, keep);
    let prefix = prefix.trim_end_matches('-');
    if prefix.is_empty() {
        hash
    } else {
        format!("{prefix}-{hash}")
    }
}

fn truncate_bytes(value: &str, max_len: usize) -> String {
    if value.len() <= max_len {
        return value.to_string();
    }
    let mut end = max_len;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}

fn short_hash(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    digest
        .iter()
        .take(4)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn is_reserved_windows_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name);
    matches!(
        stem.to_ascii_uppercase().as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_windows_names_are_rewritten() {
        assert_eq!(base_slug("CON", "course"), "con-item");
        assert_eq!(base_slug("nul.md", "course"), "nul-md");
        assert_eq!(sanitize_slug("aux", "section"), "aux-item");
        assert_eq!(sanitize_slug("COM1", "section"), "com1-item");
        assert_eq!(sanitize_slug("lpt9", "section"), "lpt9-item");
    }

    #[test]
    fn long_slugs_are_truncated_with_stable_hash() {
        let long = "a".repeat(120);
        let first = sanitize_slug(&long, "section");
        let second = sanitize_slug(&long, "section");
        assert_eq!(first, second);
        assert!(first.len() <= MAX_SLUG_LEN);
        assert!(first.contains('-'));
    }

    #[test]
    fn unique_slug_appends_suffixes_within_limit() {
        let base = "a".repeat(80);
        let mut taken = std::collections::HashSet::from([sanitize_slug(&base, "item")]);
        let next = unique_slug(&base, |candidate| taken.contains(candidate));
        assert!(next.len() <= MAX_SLUG_LEN);
        assert!(next.ends_with("-2"));
        taken.insert(next.clone());
        let third = unique_slug(&base, |candidate| taken.contains(candidate));
        assert!(third.ends_with("-3"));
    }
}
