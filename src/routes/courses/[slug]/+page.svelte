<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import CategoryPicker from '$lib/components/CategoryPicker.svelte';
  import ErrorBanner from '$lib/components/ErrorBanner.svelte';
  import ProgressBar from '$lib/components/ProgressBar.svelte';
  import SectionTree from '$lib/components/SectionTree.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import {
    createCategory,
    getCourse,
    getSection,
    listCategories,
    markSectionStatus,
    updateCourseMeta,
    type Category,
    type CourseDetail,
    type ProgressStatus,
    type SectionContent,
    type SectionNode
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
  let error = $state<string | null>(null);
  let sidebarOpen = $state(false);
  let editingTitle = $state(false);
  let titleDraft = $state('');
  let savingTitle = $state(false);
  let titleInputEl = $state<HTMLInputElement | null>(null);

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
  <main class="reader-page" class:sidebar-open={sidebarOpen}>
    <button
      type="button"
      class="reader-sidebar-backdrop"
      aria-label="Close sections"
      onclick={closeSidebar}
    ></button>

    <div class="reader-mobile-bar">
      <a class="back-link" href="/">← Library</a>
      <span class="course-title-short">{course.title}</span>
      <button type="button" class="secondary" onclick={() => (sidebarOpen = !sidebarOpen)}>
        {sidebarOpen ? 'Close' : 'Sections'}
      </button>
    </div>

    <aside class="reader-sidebar" aria-label="Course navigation">
      <div class="reader-sidebar-head">
        <a class="back-link" href="/">← Library</a>
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

      {#if course.sections.length}
        <p class="sidebar-nav-label">Sections</p>
        <SectionTree sections={course.sections} {activeSectionId} onSelect={selectSection} />
      {:else}
        <p class="muted">No sections indexed for this course.</p>
      {/if}
    </aside>

    <article class="reader-content">
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
