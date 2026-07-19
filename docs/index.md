# CourseLib

A **local-first, offline-capable** personal knowledge library for desktop.

Turn markdown — pasted in, or pulled from GitHub / GitLab / Codeberg — into navigable courses with sections, reading progress, and categories. Your **vault folder on disk** is the source of truth; SQLite is only a disposable index you can rebuild anytime.

Built with **Tauri 2** (Rust) + **SvelteKit**.

[Download the latest release](https://github.com/ririyad/courselib/releases/latest){ .md-button .md-button--primary }
[Installation guide](installation.md){ .md-button }

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

## Supported import hosts

| Host | Notes |
|------|--------|
| **GitHub** | Blob URLs → raw content; bare repo URLs resolve the default-branch `README.md` |
| **GitLab** | Raw blob paths |
| **Codeberg** | Gitea-style raw branch URLs |
| **Paste** | Any markdown pasted into the app |

Other hosts are rejected with a clear error (no silent guessing).

## What works today

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

## Next steps

- [Install CourseLib](installation.md) for your platform
- Learn the [usage flows](usage.md)
- Understand the [vault layout](vault.md)
- Find [releases and download notes](releases.md)
