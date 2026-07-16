<script lang="ts">
  import { onMount } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { getAppStatus, setVaultPath, type AppStatus } from '$lib/api';

  let status = $state<AppStatus | null>(null);
  let error = $state<string | null>(null);
  let choosing = $state(false);

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
      }
      error = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      choosing = false;
    }
  }
</script>

<main class="shell">
  <section class="hero">
    <p class="eyebrow">CourseLib</p>
    <h1>Local-first personal knowledge library</h1>
    <p class="lede">
      Milestone 0 is wired: the desktop shell starts, resolves a vault folder, and initializes Git
      metadata for future destructive-operation snapshots.
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

    <button type="button" onclick={chooseVault} disabled={choosing}>
      {choosing ? 'Choosing...' : 'Choose Vault Folder'}
    </button>
  </section>
</main>
