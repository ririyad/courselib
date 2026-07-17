<script lang="ts">
  import type { CourseProgress } from '$lib/api';

  let { progress, compact = false }: { progress: CourseProgress; compact?: boolean } = $props();

  let rounded = $derived(Math.round(progress.percent_complete));
  let width = $derived(Math.min(100, Math.max(0, progress.percent_complete)));
</script>

<div
  class:compact
  class="progress-widget"
  role="progressbar"
  aria-valuemin={0}
  aria-valuemax={100}
  aria-valuenow={rounded}
  aria-label={`${rounded}% complete`}
>
  <div class="progress-meta">
    <span>{rounded}% complete</span>
    <span>{progress.completed}/{progress.total_sections} sections</span>
  </div>
  <div class="progress-track">
    <div class="progress-fill" style={`width: ${width}%`}></div>
  </div>
</div>
