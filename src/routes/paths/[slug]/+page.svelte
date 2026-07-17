<script lang="ts">
  import { onMount } from 'svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import ErrorBanner from '$lib/components/ErrorBanner.svelte';
  import PathEditor from '$lib/components/PathEditor.svelte';
  import ProgressBar from '$lib/components/ProgressBar.svelte';
  import {
    addCourseToPath,
    getPath,
    listCourses,
    reorderPathItems,
    type CourseListItem,
    type CoursePathDetail,
    type PathOrderingItem
  } from '$lib/api';

  let path = $state<CoursePathDetail | null>(null);
  let courses = $state<CourseListItem[]>([]);
  let loading = $state(true);
  let busy = $state(false);
  let error = $state<string | null>(null);

  onMount(async () => {
    const slug = decodeURIComponent(window.location.pathname.split('/').filter(Boolean).pop() ?? '');
    try {
      const [loadedPath, loadedCourses] = await Promise.all([getPath(slug), listCourses()]);
      path = loadedPath;
      courses = loadedCourses;
      error = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      loading = false;
    }
  });

  async function addCourse(courseId: string) {
    if (!path) return;
    busy = true;
    try {
      path = await addCourseToPath(path.id, courseId);
      error = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
  }

  async function moveCourse(courseId: string, direction: -1 | 1) {
    if (!path) return;
    const index = path.courses.findIndex((item) => item.course.id === courseId);
    const target = index + direction;
    if (index < 0 || target < 0 || target >= path.courses.length) return;

    const next = [...path.courses];
    [next[index], next[target]] = [next[target], next[index]];
    await applyOrdering(next.map((item) => ({ course_id: item.course.id, optional: item.optional })));
  }

  async function toggleOptional(courseId: string, optional: boolean) {
    if (!path) return;
    await applyOrdering(
      path.courses.map((item) => ({
        course_id: item.course.id,
        optional: item.course.id === courseId ? optional : item.optional
      }))
    );
  }

  async function removeCourse(courseId: string) {
    if (!path) return;
    await applyOrdering(
      path.courses
        .filter((item) => item.course.id !== courseId)
        .map((item) => ({ course_id: item.course.id, optional: item.optional }))
    );
  }

  async function applyOrdering(ordering: PathOrderingItem[]) {
    if (!path) return;
    busy = true;
    try {
      path = await reorderPathItems(path.id, ordering);
      error = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
  }
</script>

<svelte:head>
  <title>{path ? `${path.title} · CourseLib` : 'Path · CourseLib'}</title>
</svelte:head>

<main class="page narrow">
  <a class="back-link" href="/">← Library</a>

  {#if loading}
    <p>Loading path…</p>
  {:else if error && !path}
    <EmptyState title="Path not found">
      <p class="error">{error}</p>
      <a class="button" href="/">Back to library</a>
    </EmptyState>
  {:else if path}
    <header class="path-header">
      <div>
        <p class="eyebrow">Learning path</p>
        <h1>{path.title}</h1>
      </div>
      <ProgressBar progress={path.progress} />
    </header>

    {#if error}
      <ErrorBanner message={error} />
    {/if}

    <PathEditor
      {path}
      {courses}
      {busy}
      onAdd={addCourse}
      onMove={moveCourse}
      onToggleOptional={toggleOptional}
      onRemove={removeCourse}
    />
  {/if}
</main>
