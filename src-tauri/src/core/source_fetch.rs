use anyhow::{anyhow, bail, Context, Result};
use chrono::{SecondsFormat, Utc};
use reqwest::Url;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::core::models::{CourseSource, SourceType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchedMarkdown {
    pub content: String,
    pub title_hint: Option<String>,
    pub source: CourseSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkResolution {
    Raw {
        source_type: SourceType,
        origin_url: String,
        raw_url: String,
    },
    GithubBareRepo {
        owner: String,
        repo: String,
        origin_url: String,
    },
}

#[derive(Debug, Deserialize)]
struct GithubRepoResponse {
    default_branch: String,
}

pub fn fetched_from_paste(content: String, title_hint: Option<String>) -> FetchedMarkdown {
    FetchedMarkdown {
        source: CourseSource {
            source_type: SourceType::Pasted,
            origin_url: None,
            content_hash: content_hash(&content),
            imported_at: imported_at_now(),
        },
        content,
        title_hint,
    }
}

pub async fn fetch_link(url: &str) -> Result<FetchedMarkdown> {
    let resolution = resolve_link(url)?;
    let (source_type, origin_url, raw_url) = match resolution {
        LinkResolution::Raw {
            source_type,
            origin_url,
            raw_url,
        } => (source_type, origin_url, raw_url),
        LinkResolution::GithubBareRepo {
            owner,
            repo,
            origin_url,
        } => {
            let branch = fetch_github_default_branch(&owner, &repo).await?;
            let raw_url =
                format!("https://raw.githubusercontent.com/{owner}/{repo}/{branch}/README.md");
            (SourceType::Github, origin_url, raw_url)
        }
    };

    let client = reqwest_client()?;
    let content = client
        .get(&raw_url)
        .send()
        .await
        .with_context(|| format!("failed to fetch {raw_url}"))?
        .error_for_status()
        .with_context(|| format!("source returned an error for {raw_url}"))?
        .text()
        .await
        .with_context(|| format!("failed to read response body from {raw_url}"))?;

    Ok(FetchedMarkdown {
        title_hint: None,
        source: CourseSource {
            source_type,
            origin_url: Some(origin_url),
            content_hash: content_hash(&content),
            imported_at: imported_at_now(),
        },
        content,
    })
}

pub fn resolve_link(url: &str) -> Result<LinkResolution> {
    let parsed = Url::parse(url).with_context(|| format!("invalid source URL: {url}"))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow!("source URL must include a host"))?
        .to_ascii_lowercase();
    let segments: Vec<&str> = parsed
        .path_segments()
        .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
        .unwrap_or_default();

    match host.as_str() {
        "github.com" | "www.github.com" => resolve_github(url, &segments),
        "gitlab.com" | "www.gitlab.com" => resolve_gitlab(url, &parsed, &segments),
        "codeberg.org" | "www.codeberg.org" => resolve_codeberg(url, &parsed, &segments),
        _ => bail!("unsupported source host: {host}"),
    }
}

pub fn content_hash(content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

fn resolve_github(origin_url: &str, segments: &[&str]) -> Result<LinkResolution> {
    if segments.len() < 2 {
        bail!("GitHub URL must include an owner and repository");
    }

    let owner = segments[0];
    let repo = strip_git_suffix(segments[1]);

    if segments.len() == 2 {
        return Ok(LinkResolution::GithubBareRepo {
            owner: owner.to_string(),
            repo: repo.to_string(),
            origin_url: origin_url.to_string(),
        });
    }

    if segments.get(2) == Some(&"blob") && segments.len() >= 5 {
        let branch = segments[3];
        let path = segments[4..].join("/");
        return Ok(LinkResolution::Raw {
            source_type: SourceType::Github,
            origin_url: origin_url.to_string(),
            raw_url: format!("https://raw.githubusercontent.com/{owner}/{repo}/{branch}/{path}"),
        });
    }

    if segments.get(2) == Some(&"raw") && segments.len() >= 5 {
        let branch = segments[3];
        let path = segments[4..].join("/");
        return Ok(LinkResolution::Raw {
            source_type: SourceType::Github,
            origin_url: origin_url.to_string(),
            raw_url: format!("https://raw.githubusercontent.com/{owner}/{repo}/{branch}/{path}"),
        });
    }

    bail!("unsupported GitHub URL; use a repo URL or /blob/<branch>/<path> link")
}

fn resolve_gitlab(origin_url: &str, parsed: &Url, segments: &[&str]) -> Result<LinkResolution> {
    let marker = segments
        .iter()
        .position(|segment| *segment == "-")
        .ok_or_else(|| anyhow!("unsupported GitLab URL; use a /-/blob/<branch>/<path> link"))?;

    if marker < 2 {
        bail!("GitLab URL must include a namespace and repository");
    }

    match segments.get(marker + 1) {
        Some(&"blob") if segments.len() >= marker + 4 => {
            let branch = segments[marker + 2];
            let path = segments[marker + 3..].join("/");
            let namespace = segments[..marker].join("/");
            Ok(LinkResolution::Raw {
                source_type: SourceType::Gitlab,
                origin_url: origin_url.to_string(),
                raw_url: format!(
                    "{}://{}/{}/-/raw/{}/{}",
                    parsed.scheme(),
                    parsed.host_str().unwrap_or("gitlab.com"),
                    namespace,
                    branch,
                    path
                ),
            })
        }
        Some(&"raw") if segments.len() >= marker + 4 => Ok(LinkResolution::Raw {
            source_type: SourceType::Gitlab,
            origin_url: origin_url.to_string(),
            raw_url: origin_url.to_string(),
        }),
        _ => bail!("unsupported GitLab URL; use a /-/blob/<branch>/<path> link"),
    }
}

fn resolve_codeberg(origin_url: &str, parsed: &Url, segments: &[&str]) -> Result<LinkResolution> {
    if segments.len() < 2 {
        bail!("Codeberg URL must include an owner and repository");
    }

    let owner = segments[0];
    let repo = strip_git_suffix(segments[1]);

    if segments.get(2) == Some(&"src") && segments.get(3) == Some(&"branch") && segments.len() >= 6
    {
        let branch = segments[4];
        let path = segments[5..].join("/");
        return Ok(LinkResolution::Raw {
            source_type: SourceType::Codeberg,
            origin_url: origin_url.to_string(),
            raw_url: format!(
                "{}://{}/{}/{}/raw/branch/{}/{}",
                parsed.scheme(),
                parsed.host_str().unwrap_or("codeberg.org"),
                owner,
                repo,
                branch,
                path
            ),
        });
    }

    if segments.get(2) == Some(&"raw") && segments.get(3) == Some(&"branch") && segments.len() >= 6
    {
        return Ok(LinkResolution::Raw {
            source_type: SourceType::Codeberg,
            origin_url: origin_url.to_string(),
            raw_url: origin_url.to_string(),
        });
    }

    bail!("unsupported Codeberg URL; use a /src/branch/<branch>/<path> link")
}

async fn fetch_github_default_branch(owner: &str, repo: &str) -> Result<String> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}");
    let response: GithubRepoResponse = reqwest_client()?
        .get(&url)
        .send()
        .await
        .with_context(|| format!("failed to fetch GitHub repo metadata for {owner}/{repo}"))?
        .error_for_status()
        .with_context(|| format!("GitHub repo metadata returned an error for {owner}/{repo}"))?
        .json()
        .await
        .with_context(|| format!("failed to parse GitHub repo metadata for {owner}/{repo}"))?;
    Ok(response.default_branch)
}

fn reqwest_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("CourseLib/0.1")
        .build()
        .context("failed to build HTTP client")
}

fn strip_git_suffix(repo: &str) -> &str {
    repo.strip_suffix(".git").unwrap_or(repo)
}

fn imported_at_now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_github_blob_to_raw_url() {
        let resolved =
            resolve_link("https://github.com/owner/repo/blob/main/docs/readme.md").unwrap();

        assert_eq!(
            resolved,
            LinkResolution::Raw {
                source_type: SourceType::Github,
                origin_url: "https://github.com/owner/repo/blob/main/docs/readme.md".to_string(),
                raw_url: "https://raw.githubusercontent.com/owner/repo/main/docs/readme.md"
                    .to_string(),
            }
        );
    }

    #[test]
    fn recognizes_github_bare_repo_for_default_branch_resolution() {
        let resolved = resolve_link("https://github.com/owner/repo").unwrap();

        assert_eq!(
            resolved,
            LinkResolution::GithubBareRepo {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                origin_url: "https://github.com/owner/repo".to_string(),
            }
        );
    }

    #[test]
    fn rewrites_gitlab_blob_to_raw_url() {
        let resolved =
            resolve_link("https://gitlab.com/group/project/-/blob/main/README.md").unwrap();

        assert_eq!(
            resolved,
            LinkResolution::Raw {
                source_type: SourceType::Gitlab,
                origin_url: "https://gitlab.com/group/project/-/blob/main/README.md".to_string(),
                raw_url: "https://gitlab.com/group/project/-/raw/main/README.md".to_string(),
            }
        );
    }

    #[test]
    fn rewrites_codeberg_branch_url_to_raw_url() {
        let resolved =
            resolve_link("https://codeberg.org/owner/repo/src/branch/main/docs/guide.md").unwrap();

        assert_eq!(
            resolved,
            LinkResolution::Raw {
                source_type: SourceType::Codeberg,
                origin_url: "https://codeberg.org/owner/repo/src/branch/main/docs/guide.md"
                    .to_string(),
                raw_url: "https://codeberg.org/owner/repo/raw/branch/main/docs/guide.md"
                    .to_string(),
            }
        );
    }

    #[test]
    fn rejects_unsupported_hosts() {
        let err = resolve_link("https://example.com/readme.md").unwrap_err();

        assert!(err.to_string().contains("unsupported source host"));
    }

    #[test]
    fn hashes_content_with_sha256_prefix() {
        assert_eq!(
            content_hash("abc"),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
