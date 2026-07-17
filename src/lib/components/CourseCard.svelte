<script lang="ts">
  import { onMount } from 'svelte';
  import ProgressBar from '$lib/components/ProgressBar.svelte';
  import { checkSourceDrift, type CourseListItem, type SourceDriftStatus } from '$lib/api';

  let {
    course,
    categoryNames = {}
  }: {
    course: CourseListItem;
    categoryNames?: Record<string, string>;
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
</script>

<a class="course-card" href={`/courses/${course.slug}`}>
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
