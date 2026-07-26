<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import ErrorBanner from '$lib/components/ErrorBanner.svelte';
  import {
    fetchYoutubePlaylist,
    importCourse,
    importVideoCourse,
    type LocalAttachment,
    type VideoPlaylistPreview,
    type WrittenCourse
  } from '$lib/api';

  let mode = $state<'paste' | 'link' | 'video'>('paste');
  let title = $state('');
  let markdown = $state('# My Course\n\n## Introduction\n\nStart here.');
  let url = $state('');
  let importing = $state(false);
  let error = $state<string | null>(null);
  let imported = $state<WrittenCourse | null>(null);
  let attachments = $state<LocalAttachment[]>([]);
  let markdownEl = $state<HTMLTextAreaElement | null>(null);
  let choosingImages = $state(false);
  let playlistUrl = $state('');
  let playlist = $state<VideoPlaylistPreview | null>(null);
  let fetchingPlaylist = $state(false);

  async function submit() {
    importing = true;
    imported = null;
    try {
      imported =
        mode === 'video'
          ? await importVideoCourse(title.trim(), playlistUrl.trim())
          : await importCourse(
              mode === 'paste'
                ? { Pasted: { content: markdown, title_hint: title.trim(), attachments } }
                : { Link: { url } }
            );
      error = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      importing = false;
    }
  }

  async function fetchPlaylist() {
    fetchingPlaylist = true;
    playlist = null;
    imported = null;
    try {
      playlist = await fetchYoutubePlaylist(playlistUrl.trim());
      error = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      fetchingPlaylist = false;
    }
  }

  function updatePlaylistUrl(value: string) {
    playlistUrl = value;
    playlist = null;
  }

  async function chooseImages() {
    choosingImages = true;
    try {
      const selected = await open({
        multiple: true,
        title: 'Choose course images',
        filters: [
          {
            name: 'Images',
            extensions: ['png', 'jpg', 'jpeg', 'gif', 'webp']
          }
        ]
      });
      const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
      const existing = new Set(attachments.map((attachment) => attachment.path));
      for (const path of paths) {
        if (existing.has(path)) continue;
        const name = fileName(path);
        attachments = [...attachments, { path, name }];
        existing.add(path);
        insertImageMarkdown(name);
      }
      error = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      choosingImages = false;
    }
  }

  function removeAttachment(path: string) {
    attachments = attachments.filter((attachment) => attachment.path !== path);
  }

  function insertImageMarkdown(name: string) {
    const imageMarkdown = `![${imageAlt(name)}](${name})`;
    const start = markdownEl?.selectionStart ?? markdown.length;
    const end = markdownEl?.selectionEnd ?? start;
    const before = markdown.slice(0, start);
    const after = markdown.slice(end);
    const prefix = before.length && !before.endsWith('\n') ? '\n\n' : '';
    const suffix = after.length && !after.startsWith('\n') ? '\n\n' : '';
    markdown = `${before}${prefix}${imageMarkdown}${suffix}${after}`;
    const nextPosition = before.length + prefix.length + imageMarkdown.length;
    queueMicrotask(() => {
      markdownEl?.focus();
      markdownEl?.setSelectionRange(nextPosition, nextPosition);
    });
  }

  function fileName(path: string) {
    return path.split(/[\\/]/).pop() || 'image';
  }

  function imageAlt(name: string) {
    return name.replace(/\.[^.]+$/, '').replace(/[-_]+/g, ' ');
  }
</script>

<svelte:head>
  <title>Import · CourseLib</title>
</svelte:head>

<main class="page narrow">
  <header class="section-header">
    <div>
      <p class="eyebrow">Add a course</p>
      <h1>Import</h1>
      <p class="lede">Build a course from markdown, a repository document, or a YouTube playlist.</p>
    </div>
  </header>

  <section class="card form-card">
    <div class="segmented" role="tablist" aria-label="Import method">
      <button
        type="button"
        role="tab"
        aria-selected={mode === 'paste'}
        class:active={mode === 'paste'}
        onclick={() => (mode = 'paste')}
      >
        Paste markdown
      </button>
      <button
        type="button"
        role="tab"
        aria-selected={mode === 'link'}
        class:active={mode === 'link'}
        onclick={() => (mode = 'link')}
      >
        Source link
      </button>
      <button
        type="button"
        role="tab"
        aria-selected={mode === 'video'}
        class:active={mode === 'video'}
        onclick={() => (mode = 'video')}
      >
        YouTube playlist
      </button>
    </div>

    {#if mode === 'paste'}
      <label>
        Course title <span>(required, used as the course name)</span>
        <input bind:value={title} placeholder="My Learning Notes" required />
      </label>
      <label>
        Markdown
        <textarea
          bind:this={markdownEl}
          bind:value={markdown}
          rows="16"
          placeholder={"# Course title\n\n## Section"}
        ></textarea>
      </label>
      <div class="attachment-actions">
        <button
          type="button"
          class="secondary"
          class:busy={choosingImages}
          onclick={chooseImages}
          disabled={choosingImages || importing}
        >
          {choosingImages ? 'Choosing…' : 'Attach images'}
        </button>
        <span class="muted">PNG, JPEG, GIF, or WebP · 10 MiB each</span>
      </div>
      {#if attachments.length}
        <ul class="attachment-list" aria-label="Selected image attachments">
          {#each attachments as attachment (attachment.path)}
            <li>
              <span aria-hidden="true">▧</span>
              <span class="attachment-name">{attachment.name}</span>
              <button
                type="button"
                class="ghost"
                onclick={() => removeAttachment(attachment.path)}
                disabled={importing}
                aria-label={`Remove ${attachment.name}`}
              >Remove</button>
            </li>
          {/each}
        </ul>
      {/if}
    {:else if mode === 'link'}
      <label>
        GitHub / GitLab / Codeberg markdown URL
        <input bind:value={url} placeholder="https://github.com/owner/repo/blob/main/README.md" />
      </label>
      <p class="muted">Bare GitHub repository URLs resolve to the default branch README.md.</p>
    {:else}
      <label>
        Course title <span>(required)</span>
        <input bind:value={title} placeholder="Modern JavaScript Video Course" required />
      </label>
      <label>
        YouTube playlist link
        <div class="playlist-search">
          <input
            type="url"
            value={playlistUrl}
            oninput={(event) => updatePlaylistUrl(event.currentTarget.value)}
            placeholder="https://www.youtube.com/playlist?list=…"
            required
          />
          <button
            type="button"
            class="secondary"
            class:busy={fetchingPlaylist}
            onclick={fetchPlaylist}
            disabled={fetchingPlaylist || importing || !playlistUrl.trim()}
          >{fetchingPlaylist ? 'Fetching…' : 'Fetch videos'}</button>
        </div>
      </label>
      <p class="muted">Public and unlisted playlists are supported. Videos remain hosted on YouTube.</p>

      {#if playlist}
        <section class="playlist-preview" aria-live="polite">
          <div class="playlist-preview-head">
            <div>
              <p class="eyebrow">Playlist ready</p>
              <h2>{playlist.playlist_title}</h2>
            </div>
            <strong>{playlist.video_count} video{playlist.video_count === 1 ? '' : 's'}</strong>
          </div>
          <ol class="video-preview-list">
            {#each playlist.videos as video}
              <li>
                <span class="video-preview-title">{video.title}</span>
                {#if video.duration}<span class="video-duration">{video.duration}</span>{/if}
              </li>
            {/each}
          </ol>
        </section>
      {/if}
    {/if}

    {#if error}
      <ErrorBanner message={error} />
    {/if}

    <div class="actions">
      <button
        type="button"
        class:busy={importing}
        onclick={submit}
        disabled={
          importing ||
          fetchingPlaylist ||
          (mode === 'paste'
            ? !title.trim() || !markdown.trim()
            : mode === 'link'
              ? !url.trim()
              : !title.trim() || !playlistUrl.trim() || !playlist)
        }
      >
        {importing ? 'Importing…' : mode === 'video' ? 'Create video course' : 'Import course'}
      </button>
    </div>
  </section>

  {#if imported}
    <section class="success result-card">
      <h2>Imported “{imported.title}”</h2>
      <p>
        {mode === 'video'
          ? `Added ${imported.sections.length} videos to the course.`
          : `Wrote ${imported.sections.length} top-level sections to the vault and indexed the course.`}
      </p>
      {#if imported.asset_warnings.length}
        <div class="asset-warnings" role="status">
          <strong>{imported.asset_warnings.length} image warning{imported.asset_warnings.length === 1 ? '' : 's'}</strong>
          <ul>
            {#each imported.asset_warnings as warning}
              <li>{warning}</li>
            {/each}
          </ul>
        </div>
      {/if}
      <div class="actions">
        <a class="button" href={`/courses/${imported.slug}`}>Open course</a>
      </div>
    </section>
  {/if}
</main>

<style>
  .playlist-search {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 0.65rem;
    margin-top: 0.45rem;
  }

  .playlist-search button {
    white-space: nowrap;
  }

  .playlist-preview {
    min-width: 0;
    padding: 1rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface-soft);
  }

  .playlist-preview-head {
    display: flex;
    align-items: start;
    justify-content: space-between;
    gap: 1rem;
    margin-bottom: 0.75rem;
  }

  .playlist-preview-head h2 {
    margin: 0;
    color: var(--text);
    font-size: 1rem;
    letter-spacing: 0;
    text-transform: none;
  }

  .playlist-preview-head .eyebrow {
    margin-bottom: 0.25rem;
  }

  .playlist-preview-head strong,
  .video-duration {
    color: var(--text-muted);
    font-size: 0.82rem;
    white-space: nowrap;
  }

  .video-preview-list {
    max-height: 19rem;
    overflow: auto;
    margin: 0;
    padding: 0 0 0 2rem;
  }

  .video-preview-list li {
    padding: 0.55rem 0.4rem;
    border-top: 1px solid var(--border);
  }

  .video-preview-list li::marker {
    color: var(--text-subtle);
    font-size: 0.82rem;
  }

  .video-preview-list li,
  .video-preview-title {
    min-width: 0;
  }

  .video-preview-title {
    overflow-wrap: anywhere;
  }

  .video-duration {
    float: right;
    margin-left: 0.75rem;
  }

  .attachment-actions {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex-wrap: wrap;
  }

  .attachment-list,
  .asset-warnings ul {
    margin: 0;
    padding-left: 1.25rem;
  }

  .asset-warnings,
  .asset-warnings li {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .attachment-list {
    list-style: none;
    padding-left: 0;
    display: grid;
    gap: 0.4rem;
  }

  .attachment-list li {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.45rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: 0.5rem;
  }

  .attachment-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .asset-warnings {
    margin: 0.75rem 0;
    padding: 0.75rem;
    border: 1px solid var(--warning-border, var(--border));
    border-radius: 0.5rem;
  }

  @media (max-width: 560px) {
    .playlist-search {
      grid-template-columns: 1fr;
    }

    .playlist-search button {
      width: 100%;
    }
  }
</style>
