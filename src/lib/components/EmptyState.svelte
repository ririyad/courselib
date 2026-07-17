<script lang="ts">
  let {
    title,
    children
  }: {
    title: string;
    children?: import('svelte').Snippet;
  } = $props();
</script>

<section class="empty-state">
  <h2>{title}</h2>
  {#if children}
    {@render children()}
  {/if}
</section>
