<script lang="ts">
  import ErrorBanner from '$lib/components/ErrorBanner.svelte';
  import { importCourse, type WrittenCourse } from '$lib/api';

  let mode = $state<'paste' | 'link'>('paste');
  let title = $state('');
  let markdown = $state('# My Course\n\n## Introduction\n\nStart here.');
  let url = $state('');
  let importing = $state(false);
  let error = $state<string | null>(null);
  let imported = $state<WrittenCourse | null>(null);

  async function submit() {
    importing = true;
    imported = null;
    try {
      imported = await importCourse(
        mode === 'paste'
          ? { Pasted: { content: markdown, title_hint: title.trim() } }
          : { Link: { url } }
      );
      error = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      importing = false;
    }
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
        <textarea bind:value={markdown} rows="16" placeholder={"# Course title\n\n## Section"}></textarea>
      </label>
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
      <a class="button" href={`/courses/${imported.slug}`}>Open course</a>
    </section>
  {/if}
</main>
