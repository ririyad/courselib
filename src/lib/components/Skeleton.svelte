<script lang="ts">
  let {
    variant = 'cards',
    count = 3
  }: {
    variant?: 'cards' | 'content';
    count?: number;
  } = $props();
</script>

{#if variant === 'cards'}
  <div class="skeleton-grid" aria-hidden="true">
    {#each Array(count) as _}
      <div class="skeleton-card">
        <div class="skeleton-line w-40"></div>
        <div class="skeleton-line h-title w-80"></div>
        <div class="skeleton-line w-100"></div>
        <div class="skeleton-line w-60"></div>
      </div>
    {/each}
  </div>
{:else}
  <div class="skeleton-content" aria-hidden="true">
    <div class="skeleton-line h-title w-60"></div>
    <div class="skeleton-line w-40"></div>
    <div class="skeleton-line w-100"></div>
    <div class="skeleton-line w-100"></div>
    <div class="skeleton-line w-80"></div>
    <div class="skeleton-line w-100"></div>
    <div class="skeleton-line w-60"></div>
  </div>
{/if}
