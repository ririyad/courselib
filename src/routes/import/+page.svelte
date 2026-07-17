<script lang="ts">
  import { importCourse, type WrittenCourse } from '$lib/api';

  let mode = $state<'paste' | 'link'>('paste');
  let titleHint = $state('');
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
          ? { Pasted: { content: markdown, title_hint: titleHint || null } }
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

<main class="page narrow">
  <header class="section-header">
    <div>
      <p class="eyebrow">Milestone 5</p>
      <h1>Import course</h1>
      <p class="lede">Paste markdown directly or fetch a supported repository markdown link.</p>
    </div>
    <a class="button secondary" href="/">Back to library</a>
  </header>

  <section class="card form-card">
    <div class="segmented">
      <button type="button" class:active={mode === 'paste'} onclick={() => (mode = 'paste')}>
        Paste Markdown
      </button>
      <button type="button" class:active={mode === 'link'} onclick={() => (mode = 'link')}>
        Source Link
      </button>
    </div>

    {#if mode === 'paste'}
      <label>
        Title hint <span>(optional, used when the document has no headings)</span>
        <input bind:value={titleHint} placeholder="My Learning Notes" />
      </label>
      <label>
        Markdown
        <textarea bind:value={markdown} rows="16" placeholder="# Course title\n\n## Section"></textarea>
      </label>
    {:else}
      <label>
        GitHub/GitLab/Codeberg markdown URL
        <input bind:value={url} placeholder="https://github.com/owner/repo/blob/main/README.md" />
      </label>
      <p class="muted">Bare GitHub repository URLs resolve to the default branch README.md.</p>
    {/if}

    {#if error}
      <p class="error">{error}</p>
    {/if}

    <div class="actions">
      <button type="button" onclick={submit} disabled={importing || (mode === 'paste' ? !markdown.trim() : !url.trim())}>
        {importing ? 'Importing...' : 'Import Course'}
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
