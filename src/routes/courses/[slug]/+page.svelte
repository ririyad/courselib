<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import CategoryPicker from '$lib/components/CategoryPicker.svelte';
  import ErrorBanner from '$lib/components/ErrorBanner.svelte';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import { showToast } from '$lib/stores/toasts.svelte';
  import ProgressBar from '$lib/components/ProgressBar.svelte';
  import SectionTree from '$lib/components/SectionTree.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import ThemeToggle from '$lib/components/ThemeToggle.svelte';
  import {
    checkSourceDrift,
    createCategory,
    getCourse,
    getSection,
    listCategories,
    markSectionStatus,
    reimportCourse,
    updateCourseMeta,
    type Category,
    type CourseDetail,
    type ProgressStatus,
    type SectionContent,
    type SectionNode,
    type SourceDriftStatus
  } from '$lib/api';

  let course = $state<CourseDetail | null>(null);
  let categories = $state<Category[]>([]);
  let section = $state<SectionContent | null>(null);
  let activeSectionId = $state<string | null>(null);
  let activeSectionNode = $state<SectionNode | null>(null);
  let loading = $state(true);
  let loadingSection = $state(false);
  let markingStatus = $state<ProgressStatus | null>(null);
  let updatingCategories = $state(false);
  let drift = $state<SourceDriftStatus | null>(null);
  let checkingDrift = $state(false);
  let reimporting = $state(false);
  let error = $state<string | null>(null);
  let sidebarOpen = $state(false);
  let sidebarCollapsed = $state(false);
  let editingTitle = $state(false);
  let titleDraft = $state('');
  let savingTitle = $state(false);
  let titleInputEl = $state<HTMLInputElement | null>(null);
  let confirmReimportOpen = $state(false);
  let confirmReimportMessage = $state('');

  let slug = $derived(decodeURIComponent($page.params.slug ?? ''));

  onMount(async () => {
    try {
      const [loadedCourse, loadedCategories] = await Promise.all([
        getCourse(slug),
        listCategories()
      ]);
      course = loadedCourse;
      categories = loadedCategories;
      const first = firstSection(course.sections);
      if (first) {
        await selectSection(first);
      }
      void checkDrift(false);
      error = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      loading = false;
    }
  });

  async function selectSection(node: SectionNode) {
    loadingSection = true;
    activeSectionId = node.id;
    activeSectionNode = node;
    sidebarOpen = false;
    try {
      section = await getSection(node.id);
      error = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      loadingSection = false;
    }
  }

  async function setStatus(status: ProgressStatus) {
    if (!course || !activeSectionNode) return;

    markingStatus = status;
    try {
      course.progress = await markSectionStatus(activeSectionNode.id, status);
      course.sections = updateSectionStatus(course.sections, activeSectionNode.id, status);
      activeSectionNode = findSection(course.sections, activeSectionNode.id);
      error = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      markingStatus = null;
    }
  }

  function updateSectionStatus(
    nodes: SectionNode[],
    id: string,
    status: ProgressStatus
  ): SectionNode[] {
    return nodes.map((node) => {
      if (node.id === id) {
        return {
          ...node,
          status,
          completed_at: status === 'completed' ? new Date().toISOString() : null
        };
      }
      return { ...node, children: updateSectionStatus(node.children, id, status) };
    });
  }

  async function checkDrift(showErrors = true): Promise<SourceDriftStatus | null> {
    if (!course) return null;

    checkingDrift = true;
    try {
      drift = await checkSourceDrift(course.id);
      if (showErrors) error = null;
      return drift;
    } catch (err) {
      if (showErrors) error = err instanceof Error ? err.message : String(err);
      return null;
    } finally {
      checkingDrift = false;
    }
  }

  async function requestReimport() {
    if (!course || reimporting) return;

    const status = drift ?? (await checkDrift(true));
    if (!status?.source_available) {
      error = 'This course has no source URL to re-import.';
      return;
    }

    const orphanCount = status.orphaned_progress_paths.length;
    const orphanText = orphanCount
      ? `\n\n${orphanCount} progress entr${orphanCount === 1 ? 'y' : 'ies'} will be removed because the matching section no longer exists.`
      : '';
    confirmReimportMessage = `Re-import “${course.title}” from its source? Current vault files will be committed to git before replacement.${orphanText}`;
    confirmReimportOpen = true;
  }

  function cancelReimport() {
    if (reimporting) return;
    confirmReimportOpen = false;
  }

  async function performReimport() {
    if (!course || reimporting) return;

    const activeCanonicalPath = activeSectionNode?.canonical_path ?? null;
    reimporting = true;
    try {
      const result = await reimportCourse(course.id);
      course = result.course;
      drift = null;
      const nextSection =
        (activeCanonicalPath ? findSectionByPath(course.sections, activeCanonicalPath) : null) ??
        firstSection(course.sections);
      if (nextSection) {
        await selectSection(nextSection);
      } else {
        section = null;
        activeSectionId = null;
        activeSectionNode = null;
      }
      error = null;
      confirmReimportOpen = false;
      showToast('Course re-imported from source', 'success');
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
      confirmReimportOpen = false;
    } finally {
      reimporting = false;
    }
  }

  function findSection(nodes: SectionNode[], id: string): SectionNode | null {
    for (const node of nodes) {
      if (node.id === id) return node;
      const child = findSection(node.children, id);
      if (child) return child;
    }
    return null;
  }

  async function toggleCategory(slugValue: string) {
    if (!course) return;

    const selected = new Set(course.categories);
    if (selected.has(slugValue)) {
      selected.delete(slugValue);
    } else {
      selected.add(slugValue);
    }

    updatingCategories = true;
    try {
      course = await updateCourseMeta(course.id, { categories: Array.from(selected) });
      error = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      updatingCategories = false;
    }
  }

  async function addCategory(name: string) {
    if (!course) return;

    updatingCategories = true;
    try {
      const category = await createCategory(name);
      categories = await listCategories();
      course = await updateCourseMeta(course.id, {
        categories: Array.from(new Set([...course.categories, category.slug]))
      });
      error = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
      throw err;
    } finally {
      updatingCategories = false;
    }
  }

  function startEditTitle() {
    if (!course || savingTitle) return;
    titleDraft = course.title;
    editingTitle = true;
    queueMicrotask(() => {
      titleInputEl?.focus();
      titleInputEl?.select();
    });
  }

  function cancelEditTitle() {
    if (savingTitle) return;
    editingTitle = false;
    titleDraft = course?.title ?? '';
  }

  async function saveTitle() {
    if (!course || savingTitle) return;

    const next = titleDraft.trim();
    if (!next) {
      error = 'Course title cannot be empty';
      titleDraft = course.title;
      editingTitle = false;
      return;
    }

    if (next === course.title) {
      editingTitle = false;
      return;
    }

    savingTitle = true;
    try {
      const updated = await updateCourseMeta(course.id, { title: next });
      // Keep in-memory tree/status/progress; meta response reloads from index.
      course = {
        ...updated,
        sections: course.sections,
        progress: course.progress
      };
      editingTitle = false;
      error = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
      titleDraft = course.title;
      editingTitle = false;
    } finally {
      savingTitle = false;
    }
  }

  function onTitleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter') {
      event.preventDefault();
      void saveTitle();
    } else if (event.key === 'Escape') {
      event.preventDefault();
      cancelEditTitle();
    }
  }

  function findSectionByPath(nodes: SectionNode[], canonicalPath: string): SectionNode | null {
    for (const node of nodes) {
      if (node.canonical_path === canonicalPath) return node;
      const child = findSectionByPath(node.children, canonicalPath);
      if (child) return child;
    }
    return null;
  }

  function firstSection(nodes: SectionNode[]): SectionNode | null {
    for (const node of nodes) {
      return node;
    }
    return null;
  }

  function closeSidebar() {
    sidebarOpen = false;
  }

  function onWindowKeydown(event: KeyboardEvent) {
    if (event.key !== 'Escape') return;
    if (editingTitle) {
      cancelEditTitle();
      return;
    }
    if (sidebarOpen) {
      sidebarOpen = false;
    }
  }
</script>

<svelte:window onkeydown={onWindowKeydown} />

<svelte:head>
  <title>{course ? `${course.title} · CourseLib` : 'Course · CourseLib'}</title>
</svelte:head>

{#if loading}
  <main class="reader-loading">
    <p class="muted">Loading course…</p>
    <Skeleton variant="content" />
  </main>
{:else if error && !course}
  <main class="reader-error">
    <section class="empty-state">
      <h1>Course not found</h1>
      <ErrorBanner message={error} />
      <a class="button" href="/">Back to library</a>
    </section>
  </main>
{:else if course}
  <main class="reader-page" class:sidebar-open={sidebarOpen} class:sidebar-collapsed={sidebarCollapsed}>
    <button
      type="button"
      class="reader-sidebar-backdrop"
      aria-label="Close sections"
      onclick={closeSidebar}
    ></button>

    <div class="reader-mobile-bar">
      <a class="back-link" href="/">← Library</a>
      <span class="course-title-short">{course.title}</span>
      <ThemeToggle />
      <button
        type="button"
        class="secondary"
        onclick={() => {
          sidebarCollapsed = false;
          sidebarOpen = !sidebarOpen;
        }}
      >
        {sidebarOpen ? 'Close' : 'Sections'}
      </button>
    </div>

    <aside
      class="reader-sidebar"
      aria-label="Course navigation"
      aria-hidden={sidebarCollapsed}
    >
      <div class="reader-sidebar-inner">
        <div class="reader-sidebar-actions">
          <a class="back-link" href="/">← Library</a>
          <div class="reader-sidebar-action-group">
            <ThemeToggle />
            <button
            type="button"
            class="ghost icon-button sidebar-collapse-button"
            onclick={() => (sidebarCollapsed = true)}
            aria-label="Collapse course sidebar"
            title="Collapse sidebar"
          >
            <svg
              class="sidebar-toggle-icon"
              width="16"
              height="16"
              viewBox="0 0 16 16"
              fill="none"
              aria-hidden="true"
            >
              <rect x="1.75" y="2.25" width="12.5" height="11.5" rx="2" stroke="currentColor" stroke-width="1.5" />
              <path d="M6 2.25v11.5" stroke="currentColor" stroke-width="1.5" />
              <path
                d="M10.25 5.75 8 8l2.25 2.25"
                stroke="currentColor"
                stroke-width="1.5"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </svg>
          </button>
          </div>
        </div>
        <div class="reader-sidebar-head">
          {#if editingTitle}
            <input
              class="course-title-input"
              bind:this={titleInputEl}
              bind:value={titleDraft}
              aria-label="Course title"
              disabled={savingTitle}
              onkeydown={onTitleKeydown}
              onblur={() => void saveTitle()}
            />
          {:else}
            <button
              type="button"
              class="course-title-button"
              onclick={startEditTitle}
              title="Click to rename"
              aria-label={`Rename course: ${course.title}`}
            >
              <h1>{course.title}</h1>
              <span class="course-title-hint">Edit</span>
            </button>
          {/if}
          {#if course.description}
            <p class="muted">{course.description}</p>
          {/if}
          <ProgressBar progress={course.progress} compact />
        </div>

        <details class="sidebar-disclosure">
          <summary>Categories</summary>
          <CategoryPicker
            {categories}
            selected={course.categories}
            disabled={updatingCategories}
            onToggle={toggleCategory}
            onCreate={addCategory}
          />
        </details>

        <details class="sidebar-disclosure source-panel">
          <summary>Source</summary>
          <div class="source-actions">
            {#if drift?.changed}
              <span class="drift-badge">Update available</span>
            {:else if drift && drift.source_available}
              <span class="drift-badge current">Current</span>
            {:else if drift && !drift.source_available}
              <span class="drift-badge muted-badge">Pasted source</span>
            {/if}
            <button
              type="button"
              class="ghost"
              class:busy={checkingDrift}
              onclick={() => checkDrift(true)}
              disabled={checkingDrift || reimporting}
            >{checkingDrift ? 'Checking…' : 'Check drift'}</button>
            {#if drift?.source_available}
              <button
                type="button"
                class="secondary"
                class:busy={reimporting}
                onclick={requestReimport}
                disabled={checkingDrift || reimporting}
              >{reimporting ? 'Re-importing…' : 'Re-import'}</button>
            {/if}
          </div>
          {#if drift?.changed}
            <p class="muted">The upstream source hash differs from this course snapshot.</p>
            {#if drift.orphaned_progress_paths.length}
              <p class="warning-text">
                {drift.orphaned_progress_paths.length} progress entr{drift.orphaned_progress_paths.length === 1 ? 'y' : 'ies'} will be removed on re-import.
              </p>
            {/if}
          {:else if drift && !drift.source_available}
            <p class="muted">Pasted courses do not have a remote source to check.</p>
          {/if}
        </details>

        {#if course.sections.length}
          <p class="sidebar-nav-label">Sections</p>
          <SectionTree sections={course.sections} {activeSectionId} onSelect={selectSection} />
        {:else}
          <p class="muted">No sections indexed for this course.</p>
        {/if}
      </div>
    </aside>

    <article class="reader-content">
      <button
        type="button"
        class="ghost icon-button reader-sidebar-expand"
        class:is-visible={sidebarCollapsed}
        onclick={() => (sidebarCollapsed = false)}
        aria-label="Expand course sidebar"
        title="Expand sidebar"
        tabindex={sidebarCollapsed ? 0 : -1}
      >
        <svg
          class="sidebar-toggle-icon"
          width="16"
          height="16"
          viewBox="0 0 16 16"
          fill="none"
          aria-hidden="true"
        >
          <rect x="1.75" y="2.25" width="12.5" height="11.5" rx="2" stroke="currentColor" stroke-width="1.5" />
          <path d="M6 2.25v11.5" stroke="currentColor" stroke-width="1.5" />
          <path
            d="M8 5.75 10.25 8 8 10.25"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
      </button>
      {#if error}
        <ErrorBanner message={error} />
      {/if}
      {#if loadingSection}
        <Skeleton variant="content" />
      {:else if section}
        <div class="reader-toolbar">
          <div class="reader-toolbar-meta">
            <h2>{activeSectionNode?.title ?? section.title}</h2>
            <p class="path-meta">{section.canonical_path}</p>
          </div>
          {#if activeSectionNode}
            <div
              class="status-actions"
              role="group"
              aria-label="Section progress"
            >
              <button
                type="button"
                class="status-not_started"
                class:active={activeSectionNode.status === 'not_started'}
                class:busy={markingStatus === 'not_started'}
                aria-pressed={activeSectionNode.status === 'not_started'}
                onclick={() => setStatus('not_started')}
                disabled={markingStatus !== null}
              >Not started</button>
              <button
                type="button"
                class="status-in_progress"
                class:active={activeSectionNode.status === 'in_progress'}
                class:busy={markingStatus === 'in_progress'}
                aria-pressed={activeSectionNode.status === 'in_progress'}
                onclick={() => setStatus('in_progress')}
                disabled={markingStatus !== null}
              >In progress</button>
              <button
                type="button"
                class="status-completed"
                class:active={activeSectionNode.status === 'completed'}
                class:busy={markingStatus === 'completed'}
                aria-pressed={activeSectionNode.status === 'completed'}
                onclick={() => setStatus('completed')}
                disabled={markingStatus !== null}
              >Completed</button>
            </div>
          {/if}
        </div>
        <div class="markdown-body">{@html section.html}</div>
      {:else}
        <section class="empty-state">
          <h2>Select a section</h2>
          <p>Choose a section from the tree to start reading.</p>
        </section>
      {/if}
    </article>
  </main>
{/if}

<ConfirmDialog
  open={confirmReimportOpen}
  title="Re-import course"
  message={confirmReimportMessage}
  confirmLabel={reimporting ? 'Re-importing…' : 'Re-import'}
  cancelLabel="Cancel"
  tone="danger"
  busy={reimporting}
  onConfirm={performReimport}
  onCancel={cancelReimport}
/>
