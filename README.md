# CourseLib

A **local-first, offline-capable** personal knowledge library for desktop.

Turn markdown — pasted in, or pulled from GitHub / GitLab / Codeberg — into navigable courses with sections, reading progress, and categories. Your **vault folder on disk** is the source of truth; SQLite is only a disposable index you can rebuild anytime.

Built with **Tauri 2** (Rust) + **SvelteKit**.

---

## Features

- **Import courses** from pasted markdown or a supported remote markdown URL
- **Library view** with progress bars and category filters
- **Reader** with a section tree, rendered HTML, and per-section status (not started / in progress / completed)
- **Editable course titles** and category tagging
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

- **Node.js** 20+ (npm)
- **Rust** toolchain (see `src-tauri/Cargo.toml` rust-version)
- Platform deps for [Tauri 2](https://v2.tauri.app/start/prerequisites/) (Xcode CLT on macOS, etc.)

---

## Quick start

```bash
# Install frontend dependencies
npm install

# Run the desktop app (starts Vite + Tauri)
npm run tauri -- dev
```

The app opens a native window. On first use it creates a default vault (via `dirs`) under your user data area; you can change the vault folder from **Vault settings** on the library home.

### Useful commands

| Command | Purpose |
|---------|---------|
| `npm run tauri -- dev` | Develop with hot reload |
| `npm run build` | Build static frontend → `build/` |
| `npm run tauri -- build --debug` | Package debug desktop app |
| `npm run validate` | Rust tests + frontend build + Tauri debug build |
| `cd src-tauri && cargo test` | Backend unit tests only |

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
      sections/           # heading tree as folders + markdown files
  paths/                  # curricula (backend-ready; UI later)
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
| Course reader + progress | Available |
| Categories create / assign | Available |
| Rename course title | Available |
| Paths / curricula UI | Planned |
| Search UI | Planned |
| Source drift / re-import | Planned |

Architecture, schema, IPC surface, and milestone plan live in [`AGENT.md`](./AGENT.md) for contributors.

---

## Project layout

```
courselib/
  src/                 # SvelteKit frontend
    routes/            # library, import, course reader
    lib/api.ts         # typed Tauri invoke wrappers
    lib/components/    # UI pieces
  src-tauri/           # Rust backend
    src/commands/      # IPC commands
    src/core/          # vault, parser, indexer, fetch, models
    src/db/            # SQLite schema + open helpers
  static/              # icons and static assets
  scripts/validate.sh  # full validation pipeline
```

---

## Development notes

- Frontend is a static SPA (`ssr = false`) for Tauri’s webview.
- Markdown is parsed and rendered on the **Rust** side (`comrak`) so structure and HTML stay aligned.
- Default vault path comes from the OS user dirs crate; choose another folder anytime in the UI.
- Prefer small, focused changes. Keep vault writes ahead of index updates.

---

## License

This project is licensed under the [MIT License](./LICENSE).
