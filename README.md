# CourseLib

A **local-first, offline-capable** personal knowledge library for desktop.

Turn markdown — pasted in, or pulled from GitHub / GitLab / Codeberg — into navigable courses with sections, reading progress, and categories. Your **vault folder on disk** is the source of truth; SQLite is only a disposable index you can rebuild anytime.

Built with **Tauri 2** (Rust) + **SvelteKit**.

---

## Features

- **Import courses** from pasted markdown or a supported remote markdown URL
- **Offline images** — repository images are cached in the vault; pasted courses can include local attachments
- **Library view** with progress bars, tile/list layouts, category filters, and instant metadata search
- **Reader** with a section tree, rendered HTML, and per-section status (not started / in progress / completed)
- **Editable course titles** and category tagging
- **Course deletion** with path cleanup and a vault snapshot before removal
- **Learning paths** for sequencing courses into curricula, with rolled-up progress
- **Source drift checks and manual re-import** for courses imported from supported links
- **Vault on disk** — plain files you can browse, back up, or version yourself
- Works **offline after import** (network is only used when fetching a source link)

### Supported import hosts

| Host | Notes |
|------|--------|
| **GitHub** | Blob URLs → raw content; bare repo URLs resolve the default-branch `README.md` |
| **GitLab** | Raw blob paths |
| **Codeberg** | Gitea-style raw branch URLs |
| **Paste** | Any markdown pasted into the app |

Other hosts are rejected with a clear error (no silent guessing).

---

## Requirements

- **Node.js** `^20.19.0` or `>=22.12.0` (npm)
- **Rust** toolchain (see `src-tauri/Cargo.toml` rust-version)
- Platform deps for [Tauri 2](https://v2.tauri.app/start/prerequisites/):
  - **macOS:** Xcode Command Line Tools
  - **Windows:** Visual Studio C++ Build Tools (MSVC) + Windows SDK, and the [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (included on recent Windows 10/11)

---

## Quick start

```bash
# Install frontend dependencies
npm install

# Run the desktop app (starts Vite + Tauri)
npm run tauri -- dev
```

The app opens a native window. On first use it creates a default vault under your Documents folder (`CourseLib Vault`); you can change the vault folder from **Vault settings** on the library home.

### Useful commands

| Command | Purpose |
|---------|---------|
| `npm run tauri -- dev` | Develop with hot reload |
| `npm run build` | Build static frontend → `build/` |
| `npm run tauri -- build` | Package release desktop app (local) |
| `npm run tauri -- build --debug` | Package debug desktop app |
| `npm run validate` | Rust tests + frontend build + Tauri debug build (cross-platform Node runner) |
| `cd src-tauri && cargo test` | Backend unit tests only |

---

## Releases

Published builds ship via **GitHub Releases**. CI builds:

- **macOS** arm64 and x64 on `macos-14` (native Apple Silicon + cross-compiled Intel)
- **Windows** x64 NSIS installer on `windows-latest` (unsigned)

| Surface | Purpose |
|---------|---------|
| [Releases](https://github.com/ririyad/courselib/releases) | Version notes + downloadable desktop assets |
| [Actions → Release](https://github.com/ririyad/courselib/actions/workflows/release.yml) | Full **deploy/build log** for each publish |
| [Actions → Validate](https://github.com/ririyad/courselib/actions/workflows/validate.yml) | PR/main Rust tests + frontend build (macOS + Windows) |

### Download (users)

1. Open the [latest release](https://github.com/ririyad/courselib/releases/latest).
2. Download the asset for your platform:
   - **macOS:** arm64 (Apple Silicon) or x64 (Intel)
   - **Windows:** x64 NSIS installer (`.exe`)
3. Open / install the app:
   - **macOS:** If Gatekeeper blocks an unsigned build, use **Right-click → Open** (or System Settings → Privacy & Security). Apple notarization is not configured yet.
   - **Windows:** The installer is currently **unsigned**. SmartScreen may show **Windows protected your PC** / Unknown Publisher — choose **More info → Run anyway**. WebView2 is bootstrapped on first install if missing.

### Publish a release (maintainers)

Keep versions aligned, then tag:

| File | Field |
|------|--------|
| `package.json` | `"version"` |
| `src-tauri/tauri.conf.json` | `"version"` |
| `src-tauri/Cargo.toml` | `version` (optional but recommended) |
| Git tag | `vX.Y.Z` (must match app version) |

```bash
# 1. Bump version in the files above, then commit
git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml
git commit -m "chore: release v0.1.0"

# 2. Tag and push (tag push starts the Release workflow)
git tag v0.1.0
git push origin main
git push origin v0.1.0
```

You can also run the **Release** workflow manually (**Actions → Release → Run workflow**).

**One-time repo setting:** GitHub → **Settings → Actions → General → Workflow permissions** → enable **Read and write permissions** so the workflow can create releases and upload assets.

Workflow file: [`.github/workflows/release.yml`](./.github/workflows/release.yml).

---

## How it works

```
Markdown source  →  parse headings (comrak)  →  vault files on disk
                                                      ↓
                                              SQLite index (rebuildable)
                                                      ↓
                                              UI via Tauri invoke
```

**Rule:** mutating actions write the vault first, then update the index. If the index is wrong or deleted, **Reindex vault** restores it from the folder.

### Vault layout (simplified)

```
vault/
  courses/
    <course-slug>/
      _source.md          # snapshot of the import
      _course.yaml        # title, categories, source metadata
      _progress.yaml      # section completion by canonical path
      _assets.yaml        # image source, hash, type, and ownership metadata
      assets/
        remote/           # downloaded repository images, refreshed on re-import
        local/            # user-selected attachments, preserved on re-import
      sections/           # heading tree as folders + markdown files
  paths/                  # curricula / learning paths
  categories.yaml
  .vaultgit/              # local git history for the vault
```

Section order uses numeric prefixes; progress keys use **canonical paths** (prefixes stripped) so reordering does not wipe completion as long as slugs stay stable.

---

## App surface (current)

| Area | Status |
|------|--------|
| Import (paste + URL) | Available |
| Library + category filters | Available |
| Tile / list library views | Available |
| Course metadata search | Available — title, description, and category names |
| Course reader + progress | Available |
| Categories create / assign | Available |
| Rename course title | Available |
| Course deletion | Available |
| Paths / curricula UI | Available |
| Source drift check | Available |
| Manual source re-import | Available |
| Section-content search index | Available in SQLite |
| Section-content search UI | Planned |

Contributor notes and architecture details live in this README and the Rust/Svelte source under `src-tauri/` and `src/`.

---

## Project layout

```
courselib/
  src/                 # SvelteKit frontend
    routes/            # library, import, course reader, paths
    lib/api.ts         # typed Tauri invoke wrappers
    lib/components/    # UI pieces
  src-tauri/           # Rust backend
    src/commands/      # IPC commands
    src/core/          # vault, parser, indexer, fetch, models
    src/db/            # SQLite schema + open helpers
  static/              # icons and static assets
  scripts/validate.mjs # full validation pipeline (cross-platform)
  .github/workflows/   # CI validate + release pipelines
```

---

## Development notes

- Frontend is a static SPA (`ssr = false`) for Tauri’s webview.
- Markdown is parsed and rendered on the **Rust** side (`comrak`) so structure and HTML stay aligned.
- Default vault path comes from the OS user dirs crate; choose another folder anytime in the UI.
- Image imports accept PNG, JPEG, GIF, and WebP (up to 10 MiB each). SVG is intentionally rejected.
- Remote course images are downloaded from supported repository/raw hosts so reading remains offline-capable.
- Prefer small, focused changes. Keep vault writes ahead of index updates.

---

## License

This project is licensed under the [MIT License](./LICENSE).
