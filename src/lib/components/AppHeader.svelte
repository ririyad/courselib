<script lang="ts">
  import { page } from '$app/stores';

  let pathname = $derived($page.url.pathname);
  let isImport = $derived(pathname.startsWith('/import'));
  let isHome = $derived(pathname === '/');
</script>

<header class="app-header">
  <a class="app-header-brand" href="/" aria-label="CourseLib home">
    <img src="/app-icon.svg" alt="" width="24" height="24" />
    <span>CourseLib</span>
  </a>

  <nav class="app-header-nav" aria-label="Primary">
    {#if !isHome}
      <a class="nav-link" class:active={isHome} href="/">Library</a>
    {/if}
    {#if isImport}
      <a class="nav-link active" href="/import" aria-current="page">Import</a>
    {:else}
      <a class="button" href="/import">Import</a>
    {/if}
  </nav>
</header>
