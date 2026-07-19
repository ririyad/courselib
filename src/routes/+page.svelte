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
    deleteCategory,
    deleteCourse,
    getAppStatus,
    listCategories,
    listCourses,
    reindexVault,
    renameCategory,
    setVaultPath,
    type AppStatus,
    type Category,
    type CourseListItem,
    type ReindexSummary
  } from '$lib/api';

  let status = $state<AppStatus | null>(null);
  let allCourses = $state<CourseListItem[]>([]);
  let categories = $state<Category[]>([]);
  let selectedCategory = $state<string | null>(null);
  let searchQuery = $state('');
  let reindexSummary = $state<ReindexSummary | null>(null);
  let error = $state<string | null>(null);
  let choosing = $state(false);
  let reindexing = $state(false);
  type CourseView = 'tile' | 'list';

  let loadingCourses = $state(true);
  let coursePendingDelete = $state<CourseListItem | null>(null);
  let deleting = $state(false);
  let editingCategorySlug = $state<string | null>(null);
  let categoryNameDraft = $state('');
  let renamingCategory = $state(false);
  let categoryPendingDelete = $state<Category | null>(null);
  let deletingCategory = $state(false);
  let courseView = $state<CourseView>('tile');

  let categoryNames = $derived(
    Object.fromEntries(categories.map((c) => [c.slug, c.name])) as Record<string, string>
  );

  let selectedCategoryDetails = $derived(
    selectedCategory ? categories.find((category) => category.slug === selectedCategory) : null
  );

  let normalizedSearchQuery = $derived(searchQuery.trim().toLocaleLowerCase());
  let hasSearchQuery = $derived(normalizedSearchQuery.length > 0);

  let courses = $derived(
    allCourses.filter((course) => {
      if (selectedCategory && !course.categories.includes(selectedCategory)) {
        return false;
      }

      if (!hasSearchQuery) {
        return true;
      }

      const haystack = [
        course.title,
        course.description ?? '',
        ...course.categories.map((slug) => categoryNames[slug] ?? slug)
      ]
        .join(' ')
        .toLocaleLowerCase();

      return haystack.includes(normalizedSearchQuery);
    })
  );

  let libraryHeading = $derived.by(() => {
    if (hasSearchQuery && selectedCategory) {
      return `${courses.length} result${courses.length === 1 ? '' : 's'} in filter`;
    }
    if (hasSearchQuery) {
      return `${courses.length} result${courses.length === 1 ? '' : 's'}`;
    }
    if (selectedCategory) {
      return 'Filtered';
    }
    return 'All courses';
  });

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
      allCourses = await listCourses();
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

  function selectCategory(slug: string | null) {
    selectedCategory = slug;
  }

  function clearSearch() {
    searchQuery = '';
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

  function categoryCourseCount(slug: string) {
    return allCourses.filter((course) => course.categories.includes(slug)).length;
  }

  function startCategoryRename(category: Category) {
    if (renamingCategory || deletingCategory) return;
    editingCategorySlug = category.slug;
    categoryNameDraft = category.name;
  }

  function cancelCategoryRename() {
    if (renamingCategory) return;
    editingCategorySlug = null;
    categoryNameDraft = '';
  }

  async function submitCategoryRename() {
    if (!editingCategorySlug || renamingCategory) return;

    const name = categoryNameDraft.trim();
    if (!name) {
      error = 'Category name cannot be empty.';
      return;
    }

    const previousSlug = editingCategorySlug;
    renamingCategory = true;
    try {
      const category = await renameCategory(previousSlug, name);
      if (selectedCategory === previousSlug) {
        selectedCategory = category.slug;
      }
      editingCategorySlug = null;
      categoryNameDraft = '';
      await refreshCategories();
      await refreshCourses();
      error = null;
      showToast(`Renamed category to “${category.name}”`, 'success');
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      renamingCategory = false;
    }
  }

  function requestCategoryDelete(category: Category) {
    if (renamingCategory || deletingCategory) return;
    categoryPendingDelete = category;
  }

  function cancelCategoryDelete() {
    if (deletingCategory) return;
    categoryPendingDelete = null;
  }

  async function performCategoryDelete() {
    if (!categoryPendingDelete || deletingCategory) return;

    deletingCategory = true;
    const pending = categoryPendingDelete;
    try {
      const removedFromCourses = await deleteCategory(pending.slug);
      if (selectedCategory === pending.slug) {
        selectedCategory = null;
      }
      categoryPendingDelete = null;
      await refreshCategories();
      await refreshCourses();
      error = null;
      showToast(
        removedFromCourses
          ? `Deleted “${pending.name}” and removed it from ${removedFromCourses} course${removedFromCourses === 1 ? '' : 's'}`
          : `Deleted “${pending.name}”`,
        'success'
      );
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
      categoryPendingDelete = null;
    } finally {
      deletingCategory = false;
    }
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
      <h2>{libraryHeading}</h2>
      {#if hasSearchQuery}
        <p class="sr-only" aria-live="polite">
          {courses.length} result{courses.length === 1 ? '' : 's'}
        </p>
      {/if}
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

  <div class="library-search">
    <label class="library-search-field">
      <span class="sr-only">Search courses</span>
      <svg
        class="library-search-icon"
        width="16"
        height="16"
        viewBox="0 0 16 16"
        fill="none"
        aria-hidden="true"
      >
        <circle cx="7" cy="7" r="4.25" stroke="currentColor" stroke-width="1.5" />
        <path d="M10.25 10.25 13.5 13.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
      </svg>
      <input
        type="search"
        bind:value={searchQuery}
        placeholder="Search courses…"
        aria-label="Search courses"
        autocomplete="off"
        spellcheck="false"
      />
      {#if hasSearchQuery}
        <button
          type="button"
          class="ghost icon-button library-search-clear"
          aria-label="Clear search"
          title="Clear search"
          onclick={clearSearch}
        >
          <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <path
              d="M4 4l8 8M12 4l-8 8"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
            />
          </svg>
        </button>
      {/if}
    </label>
  </div>

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
      {#each courses as course (course.id)}
        <CourseCard {course} {categoryNames} view={courseView} onDelete={requestDelete} />
      {/each}
    </div>
  {:else if allCourses.length === 0}
    <EmptyState title="No courses yet">
      <p>
        Import pasted markdown or a supported GitHub, GitLab, or Codeberg link to create your first
        course.
      </p>
      <a class="button" href="/import">Import your first course</a>
    </EmptyState>
  {:else if hasSearchQuery && selectedCategory}
    <EmptyState title="No courses found">
      <p>
        No courses match “{searchQuery.trim()}” in the selected category.
      </p>
      <div class="actions">
        <button type="button" class="secondary" onclick={clearSearch}>Clear search</button>
        <button type="button" class="ghost" onclick={() => selectCategory(null)}>Clear category</button>
      </div>
    </EmptyState>
  {:else if hasSearchQuery}
    <EmptyState title="No courses found">
      <p>No courses match “{searchQuery.trim()}”.</p>
      <button type="button" class="secondary" onclick={clearSearch}>Clear search</button>
    </EmptyState>
  {:else}
    <EmptyState title="No matches">
      <p>No courses match this category yet.</p>
      <div class="actions">
        <button type="button" class="secondary" onclick={() => selectCategory(null)}>Clear filter</button>
        {#if selectedCategoryDetails}
          <button
            type="button"
            class="ghost"
            onclick={() => startCategoryRename(selectedCategoryDetails)}
            disabled={renamingCategory || deletingCategory}
          >Rename category</button>
          <button
            type="button"
            class="danger"
            onclick={() => requestCategoryDelete(selectedCategoryDetails)}
            disabled={renamingCategory || deletingCategory}
          >Delete category</button>
        {/if}
      </div>
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

      <section class="category-manager" aria-labelledby="category-manager-heading">
        <div class="section-header compact">
          <div>
            <p class="eyebrow">Categories</p>
            <h2 id="category-manager-heading">Manage tags</h2>
          </div>
        </div>

        {#if categories.length}
          <div class="category-manager-list">
            {#each categories as category}
              <div class="category-manager-row">
                {#if editingCategorySlug === category.slug}
                  <div class="category-edit-form">
                    <label>
                      New category name
                      <input
                        bind:value={categoryNameDraft}
                        disabled={renamingCategory}
                        onkeydown={(event) => {
                          if (event.key === 'Enter') submitCategoryRename();
                          if (event.key === 'Escape') cancelCategoryRename();
                        }}
                      />
                    </label>
                    <div class="actions">
                      <button
                        type="button"
                        class:busy={renamingCategory}
                        onclick={submitCategoryRename}
                        disabled={renamingCategory || !categoryNameDraft.trim()}
                      >{renamingCategory ? 'Saving…' : 'Save'}</button>
                      <button
                        type="button"
                        class="ghost"
                        onclick={cancelCategoryRename}
                        disabled={renamingCategory}
                      >Cancel</button>
                    </div>
                  </div>
                {:else}
                  <div class="category-manager-main">
                    <strong>{category.name}</strong>
                    <span>{category.slug} · {categoryCourseCount(category.slug)} course{categoryCourseCount(category.slug) === 1 ? '' : 's'}</span>
                  </div>
                  <div class="category-manager-actions">
                    <button
                      type="button"
                      class="ghost"
                      onclick={() => startCategoryRename(category)}
                      disabled={renamingCategory || deletingCategory}
                    >Rename</button>
                    <button
                      type="button"
                      class="danger"
                      onclick={() => requestCategoryDelete(category)}
                      disabled={renamingCategory || deletingCategory}
                    >Delete</button>
                  </div>
                {/if}
              </div>
            {/each}
          </div>
        {:else}
          <p class="muted">No categories yet. Create categories from a course reader.</p>
        {/if}
      </section>
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

<ConfirmDialog
  open={categoryPendingDelete !== null}
  title="Delete category"
  message={categoryPendingDelete
    ? categoryCourseCount(categoryPendingDelete.slug)
      ? `Delete “${categoryPendingDelete.name}” and remove it from ${categoryCourseCount(categoryPendingDelete.slug)} course${categoryCourseCount(categoryPendingDelete.slug) === 1 ? '' : 's'}? Course content and progress will not be deleted.`
      : `Delete “${categoryPendingDelete.name}”? No courses use this category.`
    : 'Delete this category?'}
  confirmLabel={deletingCategory ? 'Deleting…' : 'Delete'}
  cancelLabel="Cancel"
  tone="danger"
  busy={deletingCategory}
  onConfirm={performCategoryDelete}
  onCancel={cancelCategoryDelete}
/>
