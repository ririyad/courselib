<script lang="ts">
  import { onMount } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import CourseCard from '$lib/components/CourseCard.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import ErrorBanner from '$lib/components/ErrorBanner.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import {
    createPath,
    getAppStatus,
    listCategories,
    listCourses,
    listPaths,
    reindexVault,
    setVaultPath,
    type AppStatus,
    type Category,
    type CourseListItem,
    type CoursePathSummary,
    type ReindexSummary
  } from '$lib/api';

  let status = $state<AppStatus | null>(null);
  let courses = $state<CourseListItem[]>([]);
  let paths = $state<CoursePathSummary[]>([]);
  let categories = $state<Category[]>([]);
  let selectedCategory = $state<string | null>(null);
  let newPathTitle = $state('');
  let reindexSummary = $state<ReindexSummary | null>(null);
  let error = $state<string | null>(null);
  let choosing = $state(false);
  let reindexing = $state(false);
  let creatingPath = $state(false);
  let loadingCourses = $state(true);

  let categoryNames = $derived(
    Object.fromEntries(categories.map((c) => [c.slug, c.name])) as Record<string, string>
  );

  onMount(async () => {
    await refreshStatus();
    await refreshCategories();
    await refreshCourses();
    await refreshPaths();
  });

  async function refreshStatus() {
    try {
      status = await getAppStatus();
      error = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }

  async function refreshCategories() {
    try {
      categories = await listCategories();
      if (selectedCategory && !categories.some((category) => category.slug === selectedCategory)) {
        selectedCategory = null;
      }
      error = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }

  async function refreshPaths() {
    try {
      paths = await listPaths();
      error = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }

  async function refreshCourses() {
    loadingCourses = true;
    try {
      courses = await listCourses(selectedCategory ? { category: selectedCategory } : undefined);
      error = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      loadingCourses = false;
    }
  }

  async function chooseVault() {
    choosing = true;
    try {
      const selected = await open({ directory: true, multiple: false, title: 'Choose vault folder' });
      if (typeof selected === 'string') {
        status = await setVaultPath(selected);
        reindexSummary = null;
        await refreshCategories();
        await refreshCourses();
        await refreshPaths();
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
      await refreshCategories();
      await refreshCourses();
      await refreshPaths();
      error = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      reindexing = false;
    }
  }

  async function selectCategory(slug: string | null) {
    selectedCategory = slug;
    await refreshCourses();
  }

  async function submitPath() {
    const title = newPathTitle.trim();
    if (!title) return;
    creatingPath = true;
    try {
      const path = await createPath(title);
      newPathTitle = '';
      await refreshPaths();
      window.location.href = `/paths/${path.slug}`;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      creatingPath = false;
    }
  }
</script>

<svelte:head>
  <title>Library · CourseLib</title>
</svelte:head>

<main class="page">
  <section class="library-hero">
    <p class="eyebrow">Your library</p>
    <h1>Courses</h1>
    <p class="lede">
      Import markdown, read at your pace, track progress, and sequence courses into learning paths.
    </p>
  </section>

  {#if error}
    <ErrorBanner message={error} />
  {/if}

  <section class="section-header">
    <div>
      <p class="eyebrow">Paths</p>
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

  {#if paths.length}
    <div class="path-grid">
      {#each paths as path}
        <a class="course-card" href={`/paths/${path.slug}`}>
          <div>
            <p class="card-kicker">{path.course_count} courses</p>
            <h3>{path.title}</h3>
          </div>
          <div class="progress-widget compact" aria-label={`${Math.round(path.progress.percent_complete)}% complete`}>
            <div class="progress-meta">
              <span>{Math.round(path.progress.percent_complete)}% complete</span>
              <span>{path.progress.completed}/{path.progress.total_sections} sections</span>
            </div>
            <div class="progress-track">
              <div class="progress-fill" style={`width: ${Math.min(100, Math.max(0, path.progress.percent_complete))}%`}></div>
            </div>
          </div>
        </a>
      {/each}
    </div>
  {:else}
    <EmptyState title="No paths yet">
      <p>Create one above to sequence courses into a curriculum.</p>
    </EmptyState>
  {/if}

  <section class="section-header library-section">
    <div>
      <h2>{selectedCategory ? 'Filtered' : 'All courses'}</h2>
    </div>
  </section>

  <section class="filter-bar" aria-label="Category filters">
    <button
      type="button"
      class:active={selectedCategory === null}
      onclick={() => selectCategory(null)}
    >All</button>
    {#each categories as category}
      <button
        type="button"
        class:active={selectedCategory === category.slug}
        onclick={() => selectCategory(category.slug)}
      >{category.name}</button>
    {/each}
  </section>

  {#if loadingCourses}
    <Skeleton variant="cards" count={3} />
  {:else if courses.length}
    <div class="course-grid">
      {#each courses as course}
        <CourseCard {course} {categoryNames} />
      {/each}
    </div>
  {:else}
    <EmptyState title={selectedCategory ? 'No matches' : 'No courses yet'}>
      <p>
        {selectedCategory
          ? 'No courses match this category yet.'
          : 'Import pasted markdown or a supported GitHub, GitLab, or Codeberg link to create your first course.'}
      </p>
      {#if selectedCategory}
        <button type="button" class="secondary" onclick={() => selectCategory(null)}>Clear filter</button>
      {:else}
        <a class="button" href="/import">Import your first course</a>
      {/if}
    </EmptyState>
  {/if}

  <details class="vault-details">
    <summary>Vault settings</summary>
    <div class="vault-panel">
      <div class="vault-meta">
        <div>
          <span>Path</span>
          <strong>{status?.vault_path ?? 'Checking…'}</strong>
        </div>
        <div>
          <span>Git history</span>
          <strong>{status?.vault_git_initialized ? 'Initialized' : 'Not initialized'}</strong>
        </div>
        {#if reindexSummary}
          <div>
            <span>Last reindex</span>
            <strong
              >{reindexSummary.courses} courses · {reindexSummary.sections} sections</strong
            >
          </div>
        {/if}
      </div>
      <div class="actions">
        <button
          type="button"
          class="secondary"
          class:busy={reindexing}
          onclick={runReindex}
          disabled={reindexing || choosing}
        >
          {reindexing ? 'Reindexing…' : 'Reindex vault'}
        </button>
        <button
          type="button"
          class="ghost"
          class:busy={choosing}
          onclick={chooseVault}
          disabled={choosing || reindexing}
        >
          {choosing ? 'Choosing…' : 'Choose vault folder'}
        </button>
      </div>
    </div>
  </details>
</main>
