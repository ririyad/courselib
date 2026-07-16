<script lang="ts">
  import Self from './SectionTree.svelte';
  import type { SectionNode } from '$lib/api';

  let {
    sections,
    activeSectionId,
    onSelect
  }: {
    sections: SectionNode[];
    activeSectionId: string | null;
    onSelect: (section: SectionNode) => void;
  } = $props();
</script>

<ul class="section-tree">
  {#each sections as section}
    <li>
      <button
        type="button"
        class:active={section.id === activeSectionId}
        onclick={() => onSelect(section)}
      >
        {section.title}
      </button>
      {#if section.children.length}
        <Self sections={section.children} {activeSectionId} {onSelect} />
      {/if}
    </li>
  {/each}
</ul>
