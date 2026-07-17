<script lang="ts">
  import { onMount } from 'svelte';
  import CategoryPicker from '$lib/components/CategoryPicker.svelte';
  import ProgressBar from '$lib/components/ProgressBar.svelte';
  import SectionTree from '$lib/components/SectionTree.svelte';
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

  onMount(async () => {
    const slug = decodeURIComponent(window.location.pathname.split('/').filter(Boolean).pop() ?? '');
    try {
      const [loadedCourse, loadedCategories] = await Promise.all([getCourse(slug), listCategories()]);
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

  async function toggleCategory(slug: string) {
    if (!course) return;

    const selected = new Set(course.categories);
    if (selected.has(slug)) {
      selected.delete(slug);
    } else {
      selected.add(slug);
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

  function firstSection(nodes: SectionNode[]): SectionNode | null {
    for (const node of nodes) {
      return node;
    }
    return null;
  }
</script>

<main class="reader-page">
  {#if loading}
    <p>Loading course...</p>
  {:else if error && !course}
    <section class="empty-state">
      <h1>Course not found</h1>
      <p class="error">{error}</p>
      <a class="button" href="/">Back to library</a>
    </section>
  {:else if course}
    <aside class="reader-sidebar">
      <a class="back-link" href="/">← Library</a>
      <h1>{course.title}</h1>
      {#if course.description}
        <p>{course.description}</p>
      {/if}

      <ProgressBar progress={course.progress} />

      <CategoryPicker
        {categories}
        selected={course.categories}
        disabled={updatingCategories}
        onToggle={toggleCategory}
        onCreate={addCategory}
      />

      {#if course.sections.length}
        <SectionTree sections={course.sections} {activeSectionId} onSelect={selectSection} />
      {:else}
        <p>No sections indexed for this course.</p>
      {/if}
    </aside>

    <article class="reader-content">
      {#if error}
        <p class="error">{error}</p>
      {/if}
      {#if loadingSection}
        <p>Loading section...</p>
      {:else if section}
        <div class="reader-toolbar">
          <p class="eyebrow">{section.canonical_path}</p>
          {#if activeSectionNode}
            <div class="status-actions" aria-label="Section progress">
              <button
                type="button"
                class:active={activeSectionNode.status === 'not_started'}
                onclick={() => setStatus('not_started')}
                disabled={markingStatus !== null}
              >Not started</button>
              <button
                type="button"
                class:active={activeSectionNode.status === 'in_progress'}
                onclick={() => setStatus('in_progress')}
                disabled={markingStatus !== null}
              >In progress</button>
              <button
                type="button"
                class:active={activeSectionNode.status === 'completed'}
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
          <p>Choose a section from the tree to read it.</p>
        </section>
      {/if}
    </article>
  {/if}
</main>
