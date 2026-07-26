# Releases

Published builds ship via **GitHub Releases**. CI builds:

- **macOS** arm64 and x64 on `macos-14` (native Apple Silicon + cross-compiled Intel)
- **Windows** x64 NSIS installer on `windows-latest` (unsigned)

| Surface | Purpose |
|---------|---------|
| [Releases](https://github.com/ririyad/courselib/releases) | Version notes + downloadable desktop assets |
| [Actions → Release](https://github.com/ririyad/courselib/actions/workflows/release.yml) | Full deploy/build log for each publish |
| [Actions → Validate](https://github.com/ririyad/courselib/actions/workflows/validate.yml) | PR/main Rust tests + frontend build (macOS + Windows) |
| [Actions → Docs](https://github.com/ririyad/courselib/actions/workflows/docs.yml) | Build and deploy this documentation site |

## Latest release: v0.7.0

CourseLib v0.7.0 adds video courses built from YouTube playlists:

- Fetch and preview all videos in a public or unlisted playlist
- Require a custom course title before creation
- Preserve playlist order, including playlists with more than 99 videos
- Play videos in a privacy-enhanced embedded YouTube player
- Track per-video status and aggregate progress
- Use video courses with categories and learning paths
- Persist playlist snapshots and video metadata in the local vault

[Download CourseLib v0.7.0](https://github.com/ririyad/courselib/releases/tag/v0.7.0){ .md-button .md-button--primary }

!!! note "Online playback"
    Video metadata and progress are local, but YouTube media is not downloaded. An internet connection is required to play videos.

## Download (users)

See [Installation](installation.md) for platform-specific open/install steps, including Gatekeeper and SmartScreen notes.

## Publish a release (maintainers)

Keep versions aligned, then tag:

| File | Field |
|------|--------|
| `package.json` | `"version"` |
| `src-tauri/tauri.conf.json` | `"version"` |
| `src-tauri/Cargo.toml` | `version` (optional but recommended) |
| `src-tauri/Cargo.lock` | package `courselib` version (keep in sync) |
| Git tag | `vX.Y.Z` (must match app version) |

```bash
# 1. Bump version in the files above, then commit
git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "Bump version to 0.1.0"

# 2. Tag and push (tag push starts the Release workflow)
git tag v0.1.0
git push origin main
git push origin v0.1.0
```

You can also run the **Release** workflow manually (**Actions → Release → Run workflow**).

!!! tip "One-time repo setting"
    GitHub → **Settings → Actions → General → Workflow permissions** → enable **Read and write permissions** so the workflow can create releases and upload assets.

Workflow file: [`.github/workflows/release.yml`](https://github.com/ririyad/courselib/blob/main/.github/workflows/release.yml).

## Documentation site

This site is built with [MkDocs Material](https://squidfunk.github.io/mkdocs-material/) and deployed by [`.github/workflows/docs.yml`](https://github.com/ririyad/courselib/blob/main/.github/workflows/docs.yml) to GitHub Pages.

Local preview:

```bash
python3 -m venv .venv-docs
source .venv-docs/bin/activate
pip install -r requirements-docs.txt
mkdocs serve
```
