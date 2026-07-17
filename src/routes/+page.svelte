<script lang="ts">
  import { onMount } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import CourseCard from '$lib/components/CourseCard.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import ErrorBanner from '$lib/components/ErrorBanner.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import { showToast } from '$lib/stores/toasts.svelte';
  import {
    deleteCourse,
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
  type CourseView = 'tile' | 'list';

  let loadingCourses = $state(true);
  let coursePendingDelete = $state<CourseListItem | null>(null);
  let deleting = $state(false);
  let courseView = $state<CourseView>('tile');

  let categoryNames = $derived(
    Object.fromEntries(categories.map((c) => [c.slug, c.name])) as Record<string, string>
  );

  onMount(async () => {
    const savedView = localStorage.getItem('courselib-course-view');
    if (savedView === 'tile' || savedView === 'list') {
      courseView = savedView;
    }

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
      showToast(
        `Reindexed ${reindexSummary.courses} course${reindexSummary.courses === 1 ? '' : 's'} · ${reindexSummary.sections} sections`,
        'success'
      );
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

  function setCourseView(view: CourseView) {
    courseView = view;
    localStorage.setItem('courselib-course-view', view);
  }

  function requestDelete(course: CourseListItem) {
    if (deleting) return;
    coursePendingDelete = course;
  }

  function cancelDelete() {
    if (deleting) return;
    coursePendingDelete = null;
  }

  async function performDelete() {
    if (!coursePendingDelete || deleting) return;

    deleting = true;
    const pending = coursePendingDelete;
    try {
      await deleteCourse(pending.id);
      coursePendingDelete = null;
      await refreshCourses();
      error = null;
      showToast(`Deleted “${pending.title}”`, 'success');
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
      coursePendingDelete = null;
    } finally {
      deleting = false;
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

  <section class="section-header library-section">
    <div>
      <h2>{selectedCategory ? 'Filtered' : 'All courses'}</h2>
    </div>
    <div class="segmented view-toggle" role="group" aria-label="Course view">
      <button
        type="button"
        class:active={courseView === 'tile'}
        aria-pressed={courseView === 'tile'}
        aria-label="Tile view"
        title="Tile view"
        onclick={() => setCourseView('tile')}
      >
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
          <path
            d="M2.5 2.5h4v4h-4v-4ZM9.5 2.5h4v4h-4v-4ZM2.5 9.5h4v4h-4v-4ZM9.5 9.5h4v4h-4v-4Z"
            stroke="currentColor"
            stroke-width="1.4"
            stroke-linejoin="round"
          />
        </svg>
      </button>
      <button
        type="button"
        class:active={courseView === 'list'}
        aria-pressed={courseView === 'list'}
        aria-label="List view"
        title="List view"
        onclick={() => setCourseView('list')}
      >
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
          <path
            d="M5.5 3.5h8M5.5 8h8M5.5 12.5h8M2.5 3.5h.01M2.5 8h.01M2.5 12.5h.01"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
          />
        </svg>
      </button>
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
    <div class={courseView === 'tile' ? 'course-grid' : 'course-list'}>
      {#each courses as course}
        <CourseCard {course} {categoryNames} view={courseView} onDelete={requestDelete} />
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

<ConfirmDialog
  open={coursePendingDelete !== null}
  title="Delete course"
  message={coursePendingDelete
    ? `Delete “${coursePendingDelete.title}” permanently? Vault files for this course will be removed and cannot be undone from the library.`
    : 'Delete this course permanently?'}
  confirmLabel={deleting ? 'Deleting…' : 'Delete'}
  cancelLabel="Cancel"
  tone="danger"
  busy={deleting}
  onConfirm={performDelete}
  onCancel={cancelDelete}
/>
