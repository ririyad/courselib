# CourseLib

A **local-first, offline-capable** personal knowledge library that turns the material you already have into courses you will actually want to finish.

Your next course may already be sitting in an **Obsidian vault**, a repository README, a folder of personal notes, or a conversation with an LLM. CourseLib turns that Markdown into a focused reading experience with navigation, progress, and structure. It can also turn a YouTube playlist into a trackable video course.

Your **vault folder on disk** remains the source of truth; SQLite is only a disposable index you can rebuild anytime. Built with **Tauri 2** (Rust) + **SvelteKit**.

[Download the latest release](https://github.com/ririyad/courselib/releases/latest){ .md-button .md-button--primary }
[Installation guide](installation.md){ .md-button }

---

## Bring the Markdown you already have

CourseLib does not ask you to rewrite your knowledge in a special format or lock it inside another service. If it is Markdown, it belongs here.

- **Obsidian notes** — copy the contents of any `.md` note and paste it into CourseLib. Your headings become course navigation.
- **Purpose-built LLM courses** — ask your preferred model to produce a structured Markdown course on exactly what you want to learn, then paste the result directly into the app.
- **Repository documentation** — import a README or guide from GitHub, GitLab, or Codeberg using its link.
- **Your own study notes** — rough notes, workshop material, technical guides, and knowledge dumps are all welcome. Clean headings help, but even a document without headings becomes a readable overview.

Once imported, CourseLib turns plain Markdown into an organized course with a section tree, rendered content, categories, and durable progress tracking. The source stays yours, the files stay readable, and the experience feels like a library rather than a pile of documents.

!!! tip "Generate a course with an LLM"
    Try a prompt such as: **“Create a practical, structured Markdown course about _[topic]_. Use one `#` title, clear `##` and `###` sections, examples, exercises, and a final review.”** Paste the response into **Import → Paste markdown**, give it a title, and start learning.

## Features

- **Import reading courses** from pasted markdown or a supported remote markdown URL
- **Import video courses** from public or unlisted YouTube playlists, with a complete video preview before creation
- **Embedded video player** with the same progress statuses, categories, and learning-path support as reading courses
- **Offline images** — repository images are cached in the vault; pasted courses can include local attachments
- **Library view** with progress bars, tile/list layouts, category filters, and instant metadata search
- **Reader** with a section tree, rendered HTML, and per-section status (not started / in progress / completed)
- **Editable course titles** and category tagging
- **Course deletion** with path cleanup and a vault snapshot before removal
- **Learning paths** for sequencing courses into curricula, with rolled-up progress
- **Source drift checks and manual re-import** for courses imported from supported links
- **Vault on disk** — plain files you can browse, back up, or version yourself
- Reading courses work **offline after import**; YouTube video playback requires an internet connection

## Supported import sources

| Source | Notes |
|--------|-------|
| **GitHub** | Blob URLs → raw content; bare repo URLs resolve the default-branch `README.md` |
| **GitLab** | Raw blob paths |
| **Codeberg** | Gitea-style raw branch URLs |
| **YouTube** | Public or unlisted playlist URLs; CourseLib fetches the full playlist without requiring an API key |
| **Paste** | Any markdown pasted into the app |

Unsupported hosts and invalid playlist URLs are rejected with a clear error (no silent guessing).

## What works today

| Area | Status |
|------|--------|
| Reading-course import (paste + URL) | Available |
| YouTube playlist video courses | Available |
| Embedded YouTube player | Available — internet required for playback |
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
