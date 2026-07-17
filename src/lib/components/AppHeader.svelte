<script lang="ts">
  import { page } from '$app/stores';
  import ThemeToggle from '$lib/components/ThemeToggle.svelte';

  let pathname = $derived($page.url.pathname);
  let isImport = $derived(pathname.startsWith('/import'));
  let isHome = $derived(pathname === '/');
  let isPaths = $derived(pathname.startsWith('/paths'));
</script>

<header class="app-header">
  <a class="app-header-brand" href="/" aria-label="CourseLib home">
    <img src="/app-icon.svg" alt="" width="24" height="24" />
    <span>CourseLib</span>
  </a>

  <nav class="app-header-nav" aria-label="Primary">
    <a class="nav-link" class:active={isHome} aria-current={isHome ? 'page' : undefined} href="/">
      Library
    </a>
    <a class="nav-link" class:active={isPaths} aria-current={isPaths ? 'page' : undefined} href="/paths">
      Paths
    </a>
    {#if isImport}
      <a class="nav-link active" href="/import" aria-current="page">Import</a>
    {:else}
      <a class="button" href="/import">Import</a>
    {/if}
    <ThemeToggle />
  </nav>
</header>
