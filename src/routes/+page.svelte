<script lang="ts">
  import { onMount } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import CourseCard from '$lib/components/CourseCard.svelte';
  import {
    getAppStatus,
    listCategories,
    listCourses,
    reindexVault,
    setVaultPath,
    type AppStatus,
    type Category,
    type CourseListItem,
    type ReindexSummary
  } from '$lib/api';

  let status = $state<AppStatus | null>(null);
  let courses = $state<CourseListItem[]>([]);
  let categories = $state<Category[]>([]);
  let selectedCategory = $state<string | null>(null);
  let reindexSummary = $state<ReindexSummary | null>(null);
  let error = $state<string | null>(null);
  let choosing = $state(false);
  let reindexing = $state(false);
  let loadingCourses = $state(true);

  onMount(async () => {
    await refreshStatus();
    await refreshCategories();
    await refreshCourses();
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
</script>

<main class="page">
  <section class="hero library-hero">
    <p class="eyebrow">CourseLib · Milestone 5</p>
    <h1>Local-first course library</h1>
    <p class="lede">
      Import markdown, read indexed courses, track progress, and organize your library by category.
    </p>
    <div class="actions">
      <a class="button" href="/import">Import Course</a>
      <button type="button" class="secondary" onclick={runReindex} disabled={reindexing || choosing}>
        {reindexing ? 'Reindexing...' : 'Reindex Vault'}
      </button>
      <button type="button" class="ghost" onclick={chooseVault} disabled={choosing || reindexing}>
        {choosing ? 'Choosing...' : 'Choose Vault'}
      </button>
    </div>
  </section>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  <section class="status-strip">
    <div>
      <span>Vault</span>
      <strong>{status?.vault_path ?? 'Checking...'}</strong>
    </div>
    <div>
      <span>Git</span>
      <strong>{status?.vault_git_initialized ? 'Initialized' : 'Checking...'}</strong>
    </div>
    {#if reindexSummary}
      <div>
        <span>Last reindex</span>
        <strong>{reindexSummary.courses} courses · {reindexSummary.sections} sections</strong>
      </div>
    {/if}
  </section>

  <section class="section-header">
    <div>
      <p class="eyebrow">Library</p>
      <h2>{selectedCategory ? 'Filtered courses' : 'Your courses'}</h2>
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
    <p>Loading courses...</p>
  {:else if courses.length}
    <div class="course-grid">
      {#each courses as course}
        <CourseCard {course} />
      {/each}
    </div>
  {:else}
    <section class="empty-state">
      <h2>No courses yet</h2>
      <p>
        {selectedCategory
          ? 'No courses match this category yet.'
          : 'Import pasted markdown or a supported GitHub/GitLab/Codeberg link to create your first course.'}
      </p>
      {#if selectedCategory}
        <button type="button" class="secondary" onclick={() => selectCategory(null)}>Clear filter</button>
      {:else}
        <a class="button" href="/import">Import your first course</a>
      {/if}
    </section>
  {/if}
</main>
