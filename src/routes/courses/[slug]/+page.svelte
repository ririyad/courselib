<script lang="ts">
  import { onMount } from 'svelte';
  import SectionTree from '$lib/components/SectionTree.svelte';
  import { getCourse, getSection, type CourseDetail, type SectionContent, type SectionNode } from '$lib/api';

  let course = $state<CourseDetail | null>(null);
  let section = $state<SectionContent | null>(null);
  let activeSectionId = $state<string | null>(null);
  let loading = $state(true);
  let loadingSection = $state(false);
  let error = $state<string | null>(null);

  onMount(async () => {
    const slug = decodeURIComponent(window.location.pathname.split('/').filter(Boolean).pop() ?? '');
    try {
      course = await getCourse(slug);
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
    try {
      section = await getSection(node.id);
      error = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      loadingSection = false;
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
      {#if course.categories.length}
        <div class="chips">
          {#each course.categories as category}
            <span>{category}</span>
          {/each}
        </div>
      {/if}

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
        <p class="eyebrow">{section.canonical_path}</p>
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
