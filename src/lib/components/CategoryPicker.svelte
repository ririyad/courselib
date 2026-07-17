<script lang="ts">
  import type { Category } from '$lib/api';

  let {
    categories,
    selected,
    disabled = false,
    onToggle,
    onCreate
  }: {
    categories: Category[];
    selected: string[];
    disabled?: boolean;
    onToggle: (slug: string) => void;
    onCreate: (name: string) => Promise<void> | void;
  } = $props();

  let newCategory = $state('');
  let creating = $state(false);
  let selectedSet = $derived(new Set(selected));

  async function create() {
    const name = newCategory.trim();
    if (!name) return;
    creating = true;
    try {
      await onCreate(name);
      newCategory = '';
    } finally {
      creating = false;
    }
  }
</script>

<section class="category-picker">
  <div class="category-picker-header">
    <p class="eyebrow">Categories</p>
    <span>{selected.length} selected</span>
  </div>

  {#if categories.length}
    <div class="category-options">
      {#each categories as category}
        <button
          type="button"
          class:active={selectedSet.has(category.slug)}
          onclick={() => onToggle(category.slug)}
          disabled={disabled || creating}
        >
          {category.name}
        </button>
      {/each}
    </div>
  {:else}
    <p class="muted">No categories yet. Create one below.</p>
  {/if}

  <div class="inline-create">
    <input
      bind:value={newCategory}
      placeholder="New category"
      disabled={disabled || creating}
      onkeydown={(event) => {
        if (event.key === 'Enter') create();
      }}
    />
    <button type="button" class="secondary" onclick={create} disabled={disabled || creating || !newCategory.trim()}>
      {creating ? 'Adding...' : 'Add'}
    </button>
  </div>
</section>
