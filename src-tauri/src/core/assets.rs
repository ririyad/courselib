use std::{
    collections::{HashMap, HashSet},
    fs,
    net::IpAddr,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{anyhow, bail, Context, Result};
use comrak::{
    format_commonmark,
    nodes::{AstNode, NodeValue},
    parse_document, Arena, Options,
};
use reqwest::{redirect::Policy, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::core::fs_util;

const MAX_IMAGE_COUNT: usize = 50;
const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 50 * 1024 * 1024;
const MAX_REDIRECTS: usize = 5;

#[derive(Debug, Clone, Deserialize)]
pub struct LocalAttachment {
    pub path: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssetOwnership {
    Remote,
    Local,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct AssetRecord {
    pub id: String,
    pub source: String,
    pub path: String,
    pub media_type: String,
    pub size: usize,
    pub sha256: String,
    pub ownership: AssetOwnership,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct AssetManifest {
    pub assets: Vec<AssetRecord>,
}

#[derive(Debug, Clone)]
pub struct MaterializedAssets {
    pub markdown: String,
    pub warnings: Vec<String>,
}

pub async fn materialize(
    course_path: &Path,
    course_slug: &str,
    markdown: &str,
    base_url: Option<&str>,
    attachments: &[LocalAttachment],
) -> Result<MaterializedAssets> {
    let assets_path = course_path.join("assets");
    let local_path = assets_path.join("local");
    let remote_path = assets_path.join("remote");
    let staged_remote_path = assets_path.join("remote.__staging");
    fs::create_dir_all(&local_path)
        .with_context(|| format!("failed to create {}", local_path.display()))?;
    if staged_remote_path.exists() {
        fs_util::remove_dir_all_retry(&staged_remote_path)?;
    }
    fs::create_dir_all(&staged_remote_path)
        .with_context(|| format!("failed to create {}", staged_remote_path.display()))?;

    let existing = read_manifest(course_path)?;
    let existing_remote = existing
        .assets
        .iter()
        .filter(|asset| asset.ownership == AssetOwnership::Remote)
        .filter_map(|asset| {
            Url::parse(&asset.source)
                .ok()
                .map(|url| (normalized_url(&url), asset.clone()))
        })
        .collect::<HashMap<_, _>>();
    let mut records = existing
        .assets
        .into_iter()
        .filter(|asset| asset.ownership == AssetOwnership::Local)
        .collect::<Vec<_>>();
    let mut replacements = HashMap::new();

    if attachments.len() > MAX_IMAGE_COUNT {
        bail!("course contains more than {MAX_IMAGE_COUNT} local attachments");
    }
    let mut local_bytes = 0usize;
    for attachment in attachments {
        let (record, aliases) = copy_local_attachment(&local_path, attachment)?;
        local_bytes = local_bytes.saturating_add(record.size);
        if local_bytes > MAX_TOTAL_BYTES {
            bail!(
                "local attachments exceed {} MiB in total",
                MAX_TOTAL_BYTES / 1024 / 1024
            );
        }
        let protocol_url = protocol_url(course_slug, &record.id);
        for alias in &aliases {
            if replacements.contains_key(alias) {
                bail!("multiple attachments use the same name `{alias}`");
            }
        }
        for alias in aliases {
            replacements.insert(alias, protocol_url.clone());
        }
        records.retain(|existing| existing.id != record.id);
        records.push(record);
    }

    let image_urls = collect_image_urls(markdown);
    if image_urls.len().saturating_add(attachments.len()) > MAX_IMAGE_COUNT {
        bail!("course contains more than {MAX_IMAGE_COUNT} image references");
    }

    let client = reqwest::Client::builder()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(20))
        .user_agent("CourseLib/0.5")
        .build()
        .context("failed to build image download client")?;
    let base = base_url
        .map(Url::parse)
        .transpose()
        .context("invalid image base URL")?;
    let mut warnings = Vec::new();
    let mut total_bytes = local_bytes;
    let mut downloaded_sources = HashMap::<String, AssetRecord>::new();

    for original in image_urls {
        if replacements.contains_key(&original) {
            continue;
        }

        let resolved = match resolve_image_url(&original, base.as_ref()) {
            Ok(url) => url,
            Err(err) => {
                warnings.push(format!("Skipped image `{original}`: {err}"));
                replacements.insert(original, missing_protocol_url());
                continue;
            }
        };
        let source_key = normalized_url(&resolved);
        if let Some(record) = downloaded_sources.get(&source_key) {
            replacements.insert(original, protocol_url(course_slug, &record.id));
            continue;
        }

        match download_image(&client, resolved).await {
            Ok((bytes, media_type, extension, source)) => {
                if total_bytes.saturating_add(bytes.len()) > MAX_TOTAL_BYTES {
                    warnings.push(format!(
                        "Skipped image `{original}`: total image size exceeds {} MiB",
                        MAX_TOTAL_BYTES / 1024 / 1024
                    ));
                    replacements.insert(original, missing_protocol_url());
                    continue;
                }
                total_bytes += bytes.len();
                let hash = sha256_hex(&bytes);
                let id = format!("remote-{hash}");
                let relative_path = format!("assets/remote/{hash}.{extension}");
                let staged_path = staged_remote_path.join(format!("{hash}.{extension}"));
                if !staged_path.exists() {
                    fs::write(&staged_path, &bytes)
                        .with_context(|| format!("failed to write {}", staged_path.display()))?;
                }
                let record = AssetRecord {
                    id: id.clone(),
                    source: source.clone(),
                    path: relative_path,
                    media_type,
                    size: bytes.len(),
                    sha256: format!("sha256:{hash}"),
                    ownership: AssetOwnership::Remote,
                };
                replacements.insert(original, protocol_url(course_slug, &id));
                downloaded_sources.insert(source_key, record.clone());
                records.push(record);
            }
            Err(err) => {
                if let Some(cached) = existing_remote.get(&source_key) {
                    let cached_source = course_path.join(&cached.path);
                    let file_name = Path::new(&cached.path)
                        .file_name()
                        .ok_or_else(|| anyhow!("cached image has no file name"))?;
                    let cached_destination = staged_remote_path.join(file_name);
                    fs::copy(&cached_source, &cached_destination).with_context(|| {
                        format!(
                            "failed to preserve cached image {}",
                            cached_source.display()
                        )
                    })?;
                    records.push(cached.clone());
                    downloaded_sources.insert(source_key, cached.clone());
                    replacements.insert(original.clone(), protocol_url(course_slug, &cached.id));
                    warnings.push(format!(
                        "Could not refresh image `{original}`; using cached copy: {err}"
                    ));
                } else {
                    warnings.push(format!("Skipped image `{original}`: {err}"));
                    replacements.insert(original, missing_protocol_url());
                }
            }
        }
    }

    if remote_path.exists() {
        fs_util::remove_dir_all_retry(&remote_path)?;
    }
    fs_util::rename_retry(&staged_remote_path, &remote_path)?;
    records.sort_by(|left, right| left.id.cmp(&right.id));
    write_manifest(course_path, &AssetManifest { assets: records })?;

    Ok(MaterializedAssets {
        markdown: rewrite_image_urls(markdown, &replacements)?,
        warnings,
    })
}

pub fn serve(vault_path: &Path, request_path: &str) -> Result<(Vec<u8>, String)> {
    let segments = request_path
        .trim_start_matches('/')
        .split('/')
        .collect::<Vec<_>>();
    if segments.len() != 3 || segments[0] != "course" {
        bail!("invalid asset path");
    }
    let course_slug = segments[1];
    let asset_id = segments[2];
    if course_slug.is_empty()
        || asset_id.is_empty()
        || course_slug.contains(['.', '\\'])
        || asset_id.contains(['/', '\\', '.'])
    {
        bail!("invalid asset identifier");
    }

    let course_path = vault_path.join("courses").join(course_slug);
    let manifest = read_manifest(&course_path)?;
    let record = manifest
        .assets
        .iter()
        .find(|asset| asset.id == asset_id)
        .ok_or_else(|| anyhow!("asset not found"))?;
    let requested = course_path.join(&record.path);
    if fs::symlink_metadata(&requested)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        bail!("symbolic-link assets are not served");
    }
    let canonical_course = course_path
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", course_path.display()))?;
    let canonical_requested = requested
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", requested.display()))?;
    if !canonical_requested.starts_with(&canonical_course) || !canonical_requested.is_file() {
        bail!("asset path escapes course vault");
    }
    let bytes = fs::read(&canonical_requested)
        .with_context(|| format!("failed to read {}", canonical_requested.display()))?;
    if bytes.len() != record.size {
        bail!("asset size does not match manifest");
    }
    Ok((bytes, record.media_type.clone()))
}

fn copy_local_attachment(
    local_path: &Path,
    attachment: &LocalAttachment,
) -> Result<(AssetRecord, HashSet<String>)> {
    let source_path = PathBuf::from(&attachment.path);
    let metadata = fs::metadata(&source_path)
        .with_context(|| format!("failed to read attachment {}", source_path.display()))?;
    if !metadata.is_file() {
        bail!(
            "attachment is not a regular file: {}",
            source_path.display()
        );
    }
    let bytes = fs::read(&source_path)
        .with_context(|| format!("failed to read attachment {}", source_path.display()))?;
    if bytes.len() > MAX_IMAGE_BYTES {
        bail!(
            "attachment exceeds {} MiB: {}",
            MAX_IMAGE_BYTES / 1024 / 1024,
            source_path.display()
        );
    }
    let (media_type, extension) = sniff_image(&bytes)
        .ok_or_else(|| anyhow!("unsupported image format: {}", source_path.display()))?;
    let hash = sha256_hex(&bytes);
    let id = format!("local-{hash}");
    let relative_path = format!("assets/local/{hash}.{extension}");
    let destination = local_path.join(format!("{hash}.{extension}"));
    if !destination.exists() {
        fs::write(&destination, &bytes)
            .with_context(|| format!("failed to write {}", destination.display()))?;
    }
    let file_name = source_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("attachment has no valid file name"))?
        .to_string();
    let display_name = attachment
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(&file_name)
        .to_string();
    let aliases = HashSet::from([
        file_name.clone(),
        display_name.clone(),
        format!("./{file_name}"),
        format!("./{display_name}"),
    ]);
    Ok((
        AssetRecord {
            id,
            source: display_name,
            path: relative_path,
            media_type: media_type.to_string(),
            size: bytes.len(),
            sha256: format!("sha256:{hash}"),
            ownership: AssetOwnership::Local,
        },
        aliases,
    ))
}

async fn download_image(
    client: &reqwest::Client,
    mut url: Url,
) -> Result<(Vec<u8>, String, &'static str, String)> {
    validate_image_url(&url)?;
    for redirect_count in 0..=MAX_REDIRECTS {
        let response = client
            .get(url.clone())
            .send()
            .await
            .with_context(|| format!("failed to fetch {url}"))?;
        if response.status().is_redirection() {
            if redirect_count == MAX_REDIRECTS {
                bail!("too many redirects");
            }
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| anyhow!("redirect has no valid location"))?;
            url = url.join(location).context("invalid redirect URL")?;
            validate_image_url(&url)?;
            continue;
        }
        let response = response
            .error_for_status()
            .with_context(|| format!("image returned an error for {url}"))?;
        if response.content_length().unwrap_or(0) > MAX_IMAGE_BYTES as u64 {
            bail!("image exceeds {} MiB", MAX_IMAGE_BYTES / 1024 / 1024);
        }
        let mut response = response;
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .context("failed to read image response")?
        {
            if bytes.len().saturating_add(chunk.len()) > MAX_IMAGE_BYTES {
                bail!("image exceeds {} MiB", MAX_IMAGE_BYTES / 1024 / 1024);
            }
            bytes.extend_from_slice(&chunk);
        }
        let (media_type, extension) =
            sniff_image(&bytes).ok_or_else(|| anyhow!("response is not a supported image"))?;
        return Ok((bytes, media_type.to_string(), extension, url.to_string()));
    }
    unreachable!()
}

fn resolve_image_url(value: &str, base: Option<&Url>) -> Result<Url> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        bail!("image URL is empty");
    }
    let url = if trimmed.starts_with("//") {
        Url::parse(&format!("https:{trimmed}"))?
    } else if let Ok(url) = Url::parse(trimmed) {
        url
    } else if let Some(base) = base {
        base.join(trimmed).context("invalid relative image URL")?
    } else {
        bail!("relative image has no source base URL");
    };
    validate_image_url(&url)?;
    Ok(url)
}

fn validate_image_url(url: &Url) -> Result<()> {
    if url.scheme() != "https" {
        bail!("only HTTPS images are supported");
    }
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("image URL has no host"))?
        .to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") {
        bail!("local network image hosts are not allowed");
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        if !is_public_ip(ip) {
            bail!("private network image hosts are not allowed");
        }
    }
    const ALLOWED_HOSTS: &[&str] = &[
        "github.com",
        "raw.githubusercontent.com",
        "user-images.githubusercontent.com",
        "private-user-images.githubusercontent.com",
        "gitlab.com",
        "codeberg.org",
    ];
    if !ALLOWED_HOSTS
        .iter()
        .any(|allowed| host == *allowed || host.ends_with(&format!(".{allowed}")))
    {
        bail!("image host `{host}` is not supported");
    }
    Ok(())
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_unspecified())
        }
        IpAddr::V6(ip) => !(ip.is_loopback() || ip.is_unspecified() || ip.is_unique_local()),
    }
}

fn sniff_image(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(("image/png", "png"))
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some(("image/jpeg", "jpg"))
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(("image/gif", "gif"))
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(("image/webp", "webp"))
    } else {
        None
    }
}

fn collect_image_urls(markdown: &str) -> Vec<String> {
    let arena = Arena::new();
    let options = markdown_options();
    let root = parse_document(&arena, markdown, &options);
    let mut urls = Vec::new();
    walk(root, &mut |node| {
        if let NodeValue::Image(image) = &node.data.borrow().value {
            urls.push(image.url.clone());
        }
    });
    urls
}

fn rewrite_image_urls(markdown: &str, replacements: &HashMap<String, String>) -> Result<String> {
    if replacements.is_empty() {
        return Ok(markdown.to_string());
    }
    let arena = Arena::new();
    let options = markdown_options();
    let root = parse_document(&arena, markdown, &options);
    walk(root, &mut |node| {
        let mut data = node.data.borrow_mut();
        if let NodeValue::Image(image) = &mut data.value {
            if let Some(replacement) = replacements.get(&image.url) {
                image.url.clone_from(replacement);
            }
        }
    });
    let mut output = Vec::new();
    format_commonmark(root, &options, &mut output).context("failed to rewrite image Markdown")?;
    String::from_utf8(output).context("rewritten Markdown is not UTF-8")
}

fn walk<'a>(node: &'a AstNode<'a>, callback: &mut impl FnMut(&'a AstNode<'a>)) {
    callback(node);
    for child in node.children() {
        walk(child, callback);
    }
}

fn markdown_options<'a>() -> Options<'a> {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.tasklist = true;
    options.extension.strikethrough = true;
    options.extension.autolink = true;
    options
}

fn read_manifest(course_path: &Path) -> Result<AssetManifest> {
    let path = course_path.join("_assets.yaml");
    if !path.is_file() {
        return Ok(AssetManifest::default());
    }
    let contents =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_yaml::from_str(&contents).with_context(|| format!("failed to parse {}", path.display()))
}

fn write_manifest(course_path: &Path, manifest: &AssetManifest) -> Result<()> {
    let path = course_path.join("_assets.yaml");
    let temp_path = course_path.join("_assets.yaml.tmp");
    let yaml = serde_yaml::to_string(manifest).context("failed to serialize asset manifest")?;
    fs::write(&temp_path, yaml)
        .with_context(|| format!("failed to write {}", temp_path.display()))?;
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("failed to replace {}", path.display()))?;
    }
    fs_util::rename_retry(&temp_path, &path)
}

fn protocol_url(course_slug: &str, asset_id: &str) -> String {
    format!("courselib-asset://localhost/course/{course_slug}/{asset_id}")
}

fn missing_protocol_url() -> String {
    "courselib-asset://localhost/missing".to_string()
}

fn normalized_url(url: &Url) -> String {
    let mut normalized = url.clone();
    normalized.set_fragment(None);
    normalized.to_string()
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn resolves_relative_repository_images() {
        let base =
            Url::parse("https://raw.githubusercontent.com/owner/repo/main/docs/readme.md").unwrap();
        let resolved = resolve_image_url("../images/diagram.png", Some(&base)).unwrap();
        assert_eq!(
            resolved.as_str(),
            "https://raw.githubusercontent.com/owner/repo/main/images/diagram.png"
        );
    }

    #[test]
    fn rejects_http_private_and_unapproved_hosts() {
        assert!(resolve_image_url("http://github.com/a.png", None).is_err());
        assert!(resolve_image_url("https://127.0.0.1/a.png", None).is_err());
        assert!(resolve_image_url("https://example.com/a.png", None).is_err());
    }

    #[test]
    fn rewrites_inline_and_reference_images() {
        let markdown = "![One](image.png)\n\n![Two][two]\n\n[two]: ./two.jpg\n";
        let replacements = HashMap::from([
            (
                "image.png".to_string(),
                "courselib-asset://localhost/course/c/one".to_string(),
            ),
            (
                "./two.jpg".to_string(),
                "courselib-asset://localhost/course/c/two".to_string(),
            ),
        ]);
        let rewritten = rewrite_image_urls(markdown, &replacements).unwrap();
        assert!(rewritten.contains("courselib-asset://localhost/course/c/one"));
        assert!(rewritten.contains("courselib-asset://localhost/course/c/two"));
    }

    #[test]
    fn detects_supported_raster_formats_only() {
        assert_eq!(
            sniff_image(b"\x89PNG\r\n\x1a\nrest"),
            Some(("image/png", "png"))
        );
        assert_eq!(sniff_image(b"<svg></svg>"), None);
    }

    #[test]
    fn local_attachments_are_rewritten_served_and_preserved() {
        let vault_path = std::env::temp_dir().join(format!("courselib-assets-{}", Uuid::new_v4()));
        let course_path = vault_path.join("courses").join("image-course");
        fs::create_dir_all(&course_path).unwrap();
        let source_image = vault_path.join("diagram.png");
        let image_bytes = b"\x89PNG\r\n\x1a\nfake-image-data";
        fs::write(&source_image, image_bytes).unwrap();

        let first = tauri::async_runtime::block_on(materialize(
            &course_path,
            "image-course",
            "# Images\n\n![Diagram](diagram.png)\n",
            None,
            &[LocalAttachment {
                path: source_image.to_string_lossy().into_owned(),
                name: Some("diagram.png".to_string()),
            }],
        ))
        .unwrap();
        assert!(first
            .markdown
            .contains("courselib-asset://localhost/course/image-course/local-"));
        let manifest = read_manifest(&course_path).unwrap();
        assert_eq!(manifest.assets.len(), 1);
        assert_eq!(manifest.assets[0].ownership, AssetOwnership::Local);

        let (served, media_type) = serve(
            &vault_path,
            &format!("/course/image-course/{}", manifest.assets[0].id),
        )
        .unwrap();
        assert_eq!(served, image_bytes);
        assert_eq!(media_type, "image/png");

        tauri::async_runtime::block_on(materialize(
            &course_path,
            "image-course",
            "# Images without local reference\n",
            None,
            &[],
        ))
        .unwrap();
        assert_eq!(read_manifest(&course_path).unwrap().assets.len(), 1);
        assert!(serve(&vault_path, "/course/image-course/../../settings").is_err());

        fs::remove_dir_all(&vault_path).unwrap();
    }
}
