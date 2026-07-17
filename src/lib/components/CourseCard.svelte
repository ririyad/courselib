<script lang="ts">
  import ProgressBar from '$lib/components/ProgressBar.svelte';
  import type { CourseListItem } from '$lib/api';

  let { course }: { course: CourseListItem } = $props();
</script>

<a class="course-card" href={`/courses/${course.slug}`}>
  <div>
    <p class="card-kicker">{course.section_count} sections</p>
    <h3>{course.title}</h3>
    {#if course.description}
      <p>{course.description}</p>
    {/if}
  </div>

  <ProgressBar progress={course.progress} compact />

  {#if course.categories.length}
    <div class="chips">
      {#each course.categories as category}
        <span>{category}</span>
      {/each}
    </div>
  {/if}
</a>
