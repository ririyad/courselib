<script lang="ts">
  import { onMount } from 'svelte';
  import ProgressBar from '$lib/components/ProgressBar.svelte';
  import { checkSourceDrift, type CourseListItem, type SourceDriftStatus } from '$lib/api';

  type CourseCardView = 'tile' | 'list';

  let {
    course,
    categoryNames = {},
    view = 'tile',
    onDelete
  }: {
    course: CourseListItem;
    categoryNames?: Record<string, string>;
    view?: CourseCardView;
    onDelete?: (course: CourseListItem) => void;
  } = $props();

  let labels = $derived(course.categories.map((slug) => categoryNames[slug] ?? slug));
  let drift = $state<SourceDriftStatus | null>(null);

  onMount(async () => {
    try {
      const status = await checkSourceDrift(course.id);
      if (status.changed) {
        drift = status;
      }
    } catch {
      // Drift checks are opportunistic on the library card; offline errors stay silent.
    }
  });

  function requestDelete(event: MouseEvent) {
    event.preventDefault();
    event.stopPropagation();
    onDelete?.(course);
  }
</script>

<article class="course-card" class:course-card-list={view === 'list'}>
  <a class="course-card-link" href={`/courses/${course.slug}`}>
    <div>
      <p class="card-kicker">{course.section_count} sections</p>
      <div class="card-title-row">
        <h3>{course.title}</h3>
        {#if drift?.changed}
          <span class="drift-badge">Update</span>
        {/if}
      </div>
      {#if course.description}
        <p class="card-desc">{course.description}</p>
      {/if}
    </div>

    <ProgressBar progress={course.progress} compact />

    {#if labels.length}
      <div class="chips">
        {#each labels as label}
          <span>{label}</span>
        {/each}
      </div>
    {/if}
  </a>

  {#if onDelete}
    <button
      type="button"
      class="ghost icon-button course-card-delete"
      aria-label={`Delete ${course.title}`}
      title="Delete course"
      onclick={requestDelete}
    >
      <svg
        class="course-card-delete-icon"
        width="14"
        height="14"
        viewBox="0 0 16 16"
        fill="none"
        aria-hidden="true"
      >
        <path
          d="M3.5 4.5h9M6.5 4.5V3.25A.75.75 0 0 1 7.25 2.5h1.5a.75.75 0 0 1 .75.75V4.5m1.5 0v8.25a.75.75 0 0 1-.75.75h-5.5a.75.75 0 0 1-.75-.75V4.5"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
        <path d="M7 7v4M9 7v4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
      </svg>
    </button>
  {/if}
</article>
