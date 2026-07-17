<script lang="ts">
  import ProgressBar from '$lib/components/ProgressBar.svelte';
  import type { CourseListItem } from '$lib/api';

  let {
    course,
    categoryNames = {}
  }: {
    course: CourseListItem;
    categoryNames?: Record<string, string>;
  } = $props();

  let labels = $derived(
    course.categories.map((slug) => categoryNames[slug] ?? slug)
  );
</script>

<a class="course-card" href={`/courses/${course.slug}`}>
  <div>
    <p class="card-kicker">{course.section_count} sections</p>
    <h3>{course.title}</h3>
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
