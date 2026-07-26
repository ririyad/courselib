# Usage

This page covers the main flows in the desktop app.

## Import a course

Open **Import** and choose one of three course sources:

- **Paste markdown** — paste markdown and set a title (required). You can attach local PNG, JPEG, GIF, or WebP images (up to 10 MiB each; SVG is rejected).
- **Source link** — paste a supported GitHub, GitLab, or Codeberg markdown link. CourseLib fetches the markdown and caches repository images into the vault so reading stays offline-capable.
- **YouTube playlist** — create a video course from every video in a public or unlisted playlist.

Unsupported hosts and invalid playlist links are rejected with a clear error.

### Create a YouTube video course

1. Select **YouTube playlist** on the Import page.
2. Enter the **course title**. The title is required and does not have to match the playlist title.
3. Paste a YouTube playlist URL, then select **Fetch videos**.
4. Review the playlist name, video count, titles, and durations in the generated preview.
5. Select **Create video course**.

CourseLib follows YouTube's continuation pages, so playlists longer than the first page are imported completely. The fetched preview is reused when creating the course rather than downloading the playlist a second time.

!!! info "Video availability"
    Public and unlisted playlists are supported. Private playlists cannot be imported. Video metadata and progress are stored locally, but playback uses YouTube's embedded player and requires an internet connection.

## Library

The home library shows reading and video courses together with progress bars. Cards identify video courses by their video count.

- Switch between **tile** and **list** views
- Filter by **category** chips
- Use **metadata search** to instantly filter by course title, description, and category display names

Search and category filters combine: clearing one leaves the other active.

!!! note "Search scope"
    Library search matches course metadata only. Full-text search across section content is indexed in SQLite but does not yet have a UI.

## Reader

Open a course to:

- Browse the **section tree** for reading courses or the **video list** for video courses
- Read rendered HTML or play the selected video in YouTube's privacy-enhanced embedded player
- Open an embedded video on YouTube when needed
- Mark sections or videos as **not started**, **in progress**, or **completed**
- Track overall completion in the course progress bar
- Edit the **course title**
- Assign **categories**

Video durations appear in the course navigation. Selecting another video replaces the player while preserving the status of every video independently.

## Categories

Create, rename, and delete categories from the library. Assign categories on the course reader. Category membership drives library filter chips and metadata search.

## Learning paths

Use **Paths** to sequence courses into curricula:

- Create a path
- Add courses
- Reorder items
- Track rolled-up progress across the path

## Source drift and re-import

For reading courses imported from a supported repository URL:

- CourseLib can **check source drift** when the upstream markdown hash differs from your vault snapshot
- **Manual re-import** refreshes content from the source

YouTube playlists are imported as snapshots. Playlist drift checks and playlist refresh are not currently available; importing the playlist again creates a new video course.

!!! warning "Re-import and progress"
    Re-import may orphan progress if section paths change. Review the confirmation carefully before continuing.

## Delete a course

Delete from the library. CourseLib takes a vault snapshot before removal, then cleans up the course folder and index.

## Theme

Toggle light, dark, or system theme from the app header.
