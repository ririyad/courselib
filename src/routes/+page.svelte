<script lang="ts">
  import { onMount } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { getAppStatus, reindexVault, setVaultPath, type AppStatus, type ReindexSummary } from '$lib/api';

  let status = $state<AppStatus | null>(null);
  let reindexSummary = $state<ReindexSummary | null>(null);
  let error = $state<string | null>(null);
  let choosing = $state(false);
  let reindexing = $state(false);

  onMount(async () => {
    await refreshStatus();
  });

  async function refreshStatus() {
    try {
      status = await getAppStatus();
      error = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }

  async function chooseVault() {
    choosing = true;
    try {
      const selected = await open({ directory: true, multiple: false, title: 'Choose vault folder' });
      if (typeof selected === 'string') {
        status = await setVaultPath(selected);
        reindexSummary = null;
      }
      error = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      choosing = false;
    }
  }

  async function runReindex() {
    reindexing = true;
    try {
      reindexSummary = await reindexVault();
      error = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      reindexing = false;
    }
  }
</script>

<main class="shell">
  <section class="hero">
    <p class="eyebrow">CourseLib</p>
    <h1>Local-first personal knowledge library</h1>
    <p class="lede">
      Milestone 2 is wired: the app creates the vault, initializes Git metadata, applies the SQLite
      schema, and can rebuild the disposable index from plain files on disk.
    </p>
  </section>

  <section class="card" aria-live="polite">
    <h2>Vault Status</h2>
    {#if error}
      <p class="error">{error}</p>
    {:else if status}
      <dl>
        <div>
          <dt>Vault path</dt>
          <dd>{status.vault_path}</dd>
        </div>
        <div>
          <dt>Courses folder</dt>
          <dd>{status.courses_dir_exists ? 'Ready' : 'Missing'}</dd>
        </div>
        <div>
          <dt>Paths folder</dt>
          <dd>{status.paths_dir_exists ? 'Ready' : 'Missing'}</dd>
        </div>
        <div>
          <dt>Git metadata</dt>
          <dd>{status.vault_git_initialized ? 'Initialized' : 'Missing'}</dd>
        </div>
      </dl>
    {:else}
      <p>Checking vault...</p>
    {/if}

    <div class="actions">
      <button type="button" onclick={chooseVault} disabled={choosing || reindexing}>
        {choosing ? 'Choosing...' : 'Choose Vault Folder'}
      </button>
      <button type="button" class="secondary" onclick={runReindex} disabled={reindexing || choosing}>
        {reindexing ? 'Reindexing...' : 'Reindex Vault'}
      </button>
    </div>

    {#if reindexSummary}
      <p class="success">
        Indexed {reindexSummary.courses} courses, {reindexSummary.sections} sections,
        {reindexSummary.categories} categories, and {reindexSummary.paths} paths.
      </p>
    {/if}
  </section>
</main>
