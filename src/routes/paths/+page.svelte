<script lang="ts">
  import { onMount } from 'svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import ErrorBanner from '$lib/components/ErrorBanner.svelte';
  import ProgressBar from '$lib/components/ProgressBar.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import { createPath, listPaths, type CoursePathSummary } from '$lib/api';

  let paths = $state<CoursePathSummary[]>([]);
  let newPathTitle = $state('');
  let loading = $state(true);
  let creatingPath = $state(false);
  let error = $state<string | null>(null);

  onMount(async () => {
    await refreshPaths();
  });

  async function refreshPaths() {
    loading = true;
    try {
      paths = await listPaths();
      error = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      loading = false;
    }
  }

  async function submitPath() {
    const title = newPathTitle.trim();
    if (!title) return;

    creatingPath = true;
    try {
      const path = await createPath(title);
      newPathTitle = '';
      window.location.href = `/paths/${path.slug}`;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      creatingPath = false;
    }
  }
</script>

<svelte:head>
  <title>Paths · CourseLib</title>
</svelte:head>

<main class="page">
  <section class="library-hero">
    <p class="eyebrow">Curriculum</p>
    <h1>Course paths</h1>
    <p class="lede">Sequence existing courses into focused learning paths and track rolled-up progress.</p>
  </section>

  {#if error}
    <ErrorBanner message={error} />
  {/if}

  <section class="section-header">
    <div>
      <h2>Learning paths</h2>
    </div>
    <div class="inline-create path-create">
      <input
        bind:value={newPathTitle}
        placeholder="New path title"
        disabled={creatingPath}
        onkeydown={(event) => {
          if (event.key === 'Enter') submitPath();
        }}
      />
      <button type="button" onclick={submitPath} disabled={creatingPath || !newPathTitle.trim()}>
        {creatingPath ? 'Creating…' : 'Create path'}
      </button>
    </div>
  </section>

  {#if loading}
    <Skeleton variant="cards" count={3} />
  {:else if paths.length}
    <div class="path-grid">
      {#each paths as path}
        <a class="course-card" href={`/paths/${path.slug}`}>
          <div>
            <p class="card-kicker">{path.course_count} courses</p>
            <h3>{path.title}</h3>
          </div>
          <ProgressBar progress={path.progress} compact />
        </a>
      {/each}
    </div>
  {:else}
    <EmptyState title="No paths yet">
      <p>Create one above to sequence courses into a curriculum.</p>
    </EmptyState>
  {/if}
</main>
