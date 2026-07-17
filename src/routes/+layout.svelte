<script lang="ts">
  import '../app.css';
  import { page } from '$app/stores';
  import AppHeader from '$lib/components/AppHeader.svelte';

  let { children } = $props();

  let isReader = $derived($page.url.pathname.startsWith('/courses/'));
</script>

{#if isReader}
  {@render children()}
{:else}
  <div class="app-shell">
    <AppHeader />
    <div class="app-main">
      {@render children()}
    </div>
  </div>
{/if}
