<script lang="ts">
  import type { CourseListItem, CoursePathDetail } from '$lib/api';

  let {
    path,
    courses,
    busy = false,
    onAdd,
    onMove,
    onToggleOptional,
    onRemove
  }: {
    path: CoursePathDetail;
    courses: CourseListItem[];
    busy?: boolean;
    onAdd: (courseId: string) => Promise<void> | void;
    onMove: (courseId: string, direction: -1 | 1) => Promise<void> | void;
    onToggleOptional: (courseId: string, optional: boolean) => Promise<void> | void;
    onRemove: (courseId: string) => Promise<void> | void;
  } = $props();

  let selectedCourseId = $state('');
  let pathCourseIds = $derived(new Set(path.courses.map((item) => item.course.id)));
  let availableCourses = $derived(courses.filter((course) => !pathCourseIds.has(course.id)));

  async function addSelected() {
    if (!selectedCourseId) return;
    await onAdd(selectedCourseId);
    selectedCourseId = '';
  }
</script>

<section class="path-editor">
  <div class="section-header compact">
    <div>
      <p class="eyebrow">Path editor</p>
      <h2>Courses</h2>
    </div>
  </div>

  <div class="inline-create">
    <select bind:value={selectedCourseId} disabled={busy || availableCourses.length === 0}>
      <option value="">{availableCourses.length ? 'Choose a course' : 'All courses added'}</option>
      {#each availableCourses as course}
        <option value={course.id}>{course.title}</option>
      {/each}
    </select>
    <button type="button" onclick={addSelected} disabled={busy || !selectedCourseId}>Add</button>
  </div>

  {#if path.courses.length}
    <ol class="path-items">
      {#each path.courses as item, index}
        <li>
          <div>
            <strong>{item.course.title}</strong>
            <span>{item.course.section_count} sections · {Math.round(item.course.progress.percent_complete)}% complete</span>
          </div>
          <div class="path-item-actions">
            <label class="check-label">
              <input
                type="checkbox"
                checked={item.optional}
                disabled={busy}
                onchange={(event) => onToggleOptional(item.course.id, event.currentTarget.checked)}
              />
              Optional
            </label>
            <button type="button" class="ghost" onclick={() => onMove(item.course.id, -1)} disabled={busy || index === 0}>↑</button>
            <button type="button" class="ghost" onclick={() => onMove(item.course.id, 1)} disabled={busy || index === path.courses.length - 1}>↓</button>
            <button type="button" class="secondary" onclick={() => onRemove(item.course.id)} disabled={busy}>Remove</button>
          </div>
        </li>
      {/each}
    </ol>
  {:else}
    <p class="muted">Add courses to build this path.</p>
  {/if}
</section>
