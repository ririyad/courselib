<script lang="ts">
  import Self from './SectionTree.svelte';
  import type { ProgressStatus, SectionNode } from '$lib/api';

  let {
    sections,
    activeSectionId,
    onSelect
  }: {
    sections: SectionNode[];
    activeSectionId: string | null;
    onSelect: (section: SectionNode) => void;
  } = $props();

  const statusLabels: Record<ProgressStatus, string> = {
    not_started: 'Not started',
    in_progress: 'In progress',
    completed: 'Completed'
  };
</script>

<ul class="section-tree">
  {#each sections as section}
    <li>
      <button
        type="button"
        class:active={section.id === activeSectionId}
        onclick={() => onSelect(section)}
      >
        <span>{section.title}</span>
        <span
          class={`status-dot ${section.status}`}
          aria-label={statusLabels[section.status]}
          title={statusLabels[section.status]}
        ></span>
      </button>
      {#if section.children.length}
        <Self sections={section.children} {activeSectionId} {onSelect} />
      {/if}
    </li>
  {/each}
</ul>
