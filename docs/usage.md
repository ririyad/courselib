# Usage

This page covers the main flows in the desktop app.

## Import a course

Open **Import** and choose one of:

- **Paste** — paste markdown and set a title (required). You can attach local PNG, JPEG, GIF, or WebP images (up to 10 MiB each; SVG is rejected).
- **URL** — paste a supported GitHub, GitLab, or Codeberg markdown link. CourseLib fetches the markdown and caches repository images into the vault so reading stays offline-capable.

Unsupported hosts are rejected with a clear error.

## Library

The home library shows your courses with progress bars.

- Switch between **tile** and **list** views
- Filter by **category** chips
- Use **metadata search** to instantly filter by course title, description, and category display names

Search and category filters combine: clearing one leaves the other active.

!!! note "Search scope"
    Library search matches course metadata only. Full-text search across section content is indexed in SQLite but does not yet have a UI.

## Reader

Open a course to:

- Browse the **section tree**
- Read rendered HTML
- Mark sections as **not started**, **in progress**, or **completed**
- Edit the **course title**
- Assign **categories**

## Categories

Create, rename, and delete categories from the library. Assign categories on the course reader. Category membership drives library filter chips and metadata search.

## Learning paths

Use **Paths** to sequence courses into curricula:

- Create a path
- Add courses
- Reorder items
- Track rolled-up progress across the path

## Source drift and re-import

For courses imported from a supported URL:

- CourseLib can **check source drift** when the upstream markdown hash differs from your vault snapshot
- **Manual re-import** refreshes content from the source

!!! warning "Re-import and progress"
    Re-import may orphan progress if section paths change. Review the confirmation carefully before continuing.

## Delete a course

Delete from the library. CourseLib takes a vault snapshot before removal, then cleans up the course folder and index.

## Theme

Toggle light, dark, or system theme from the app header.
