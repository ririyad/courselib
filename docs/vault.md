# Vault & how it works

CourseLib is **local-first**: the vault folder on disk is the source of truth. SQLite is only a disposable index you can rebuild anytime.

## Pipeline

```text
Markdown source  →  parse headings (comrak) ─┐
                                              ├→ vault files on disk
YouTube playlist →  fetch video metadata ─────┘          ↓
                                                  SQLite index (rebuildable)
                                                          ↓
                                                  UI via Tauri invoke
```

**Rule:** mutating actions write the vault first, then update the index. If the index is wrong or deleted, **Reindex vault** restores it from the folder.

## Default location

On first launch the app creates a vault under your Documents folder (`CourseLib Vault`). Change it anytime from **Vault settings** on the library home.

Back up by copying the vault folder (or versioning it yourself). The vault includes a local git history under `.vaultgit/`.

## Vault layout (simplified)

```text
vault/
  courses/
    <reading-course>/
      _source.md          # markdown import snapshot
      _course.yaml        # title, type, categories, source metadata
      _progress.yaml      # completion by canonical path
      _assets.yaml        # image source, hash, type, and ownership metadata
      assets/
        remote/           # downloaded repository images, refreshed on re-import
        local/            # user-selected attachments, preserved on re-import
      sections/           # heading tree as folders + markdown files
    <video-course>/
      _source.json        # YouTube playlist metadata snapshot
      _course.yaml        # title, video type, categories, playlist source
      _progress.yaml      # completion for each video
      _videos.yaml        # video IDs, URLs, titles, durations, and thumbnails
      sections/           # one ordered markdown entry per video
  paths/                  # curricula / learning paths
  categories.yaml
  .vaultgit/              # local git history for the vault
```

Section and video order use numeric prefixes; progress keys use **canonical paths** (prefixes stripped) so reindexing does not wipe completion as long as slugs stay stable. Video courses use wider numeric prefixes so playlists with more than 99 videos retain their order.

## Images

- Remote course images are downloaded from supported repository/raw hosts into `assets/remote/`
- Pasted courses can include user-selected attachments in `assets/local/`
- Accepted formats: PNG, JPEG, GIF, WebP (up to 10 MiB each)
- SVG is intentionally rejected

## Video courses

CourseLib stores playlist and video metadata in the vault, but it does not download YouTube media. The reader reconstructs each course from `_videos.yaml` and opens the selected video with YouTube's privacy-enhanced embed URL. Consequently, titles, ordering, categories, and progress survive an index rebuild, while playback still requires network access.

## Reindex

If the library looks wrong after moving files, switching machines, or recovering from a backup, use **Reindex vault**. That rebuilds SQLite from the on-disk vault without changing your markdown, video metadata, or progress files.
