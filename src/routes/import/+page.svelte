<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import ErrorBanner from '$lib/components/ErrorBanner.svelte';
  import { importCourse, type LocalAttachment, type WrittenCourse } from '$lib/api';

  let mode = $state<'paste' | 'link'>('paste');
  let title = $state('');
  let markdown = $state('# My Course\n\n## Introduction\n\nStart here.');
  let url = $state('');
  let importing = $state(false);
  let error = $state<string | null>(null);
  let imported = $state<WrittenCourse | null>(null);
  let attachments = $state<LocalAttachment[]>([]);
  let markdownEl = $state<HTMLTextAreaElement | null>(null);
  let choosingImages = $state(false);

  async function submit() {
    importing = true;
    imported = null;
    try {
      imported = await importCourse(
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
      <p class="lede">Paste markdown or fetch a supported repository link.</p>
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
    {:else}
      <label>
        GitHub / GitLab / Codeberg markdown URL
        <input bind:value={url} placeholder="https://github.com/owner/repo/blob/main/README.md" />
      </label>
      <p class="muted">Bare GitHub repository URLs resolve to the default branch README.md.</p>
    {/if}

    {#if error}
      <ErrorBanner message={error} />
    {/if}

    <div class="actions">
      <button
        type="button"
        class:busy={importing}
        onclick={submit}
        disabled={importing || (mode === 'paste' ? !title.trim() || !markdown.trim() : !url.trim())}
      >
        {importing ? 'Importing…' : 'Import course'}
      </button>
    </div>
  </section>

  {#if imported}
    <section class="success result-card">
      <h2>Imported “{imported.title}”</h2>
      <p>Wrote {imported.sections.length} top-level sections to the vault and indexed the course.</p>
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
      <a class="button" href={`/courses/${imported.slug}`}>Open course</a>
    </section>
  {/if}
</main>

<style>
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
</style>
