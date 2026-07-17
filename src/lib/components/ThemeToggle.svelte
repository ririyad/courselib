<script lang="ts">
  import { onMount } from 'svelte';
  import {
    applyMode,
    getStoredMode,
    nextMode,
    systemPrefersDark,
    type ThemeMode
  } from '$lib/theme';

  let mode = $state<ThemeMode>('system');
  let systemDark = $state(false);

  let title = $derived(
    mode === 'system'
      ? `Theme: System (${systemDark ? 'dark' : 'light'})`
      : `Theme: ${mode === 'dark' ? 'Dark' : 'Light'}`
  );

  onMount(() => {
    mode = getStoredMode();
    systemDark = systemPrefersDark();

    const query = window.matchMedia('(prefers-color-scheme: dark)');
    const onChange = (event: MediaQueryListEvent) => {
      systemDark = event.matches;
    };
    query.addEventListener('change', onChange);
    return () => query.removeEventListener('change', onChange);
  });

  function cycle() {
    mode = nextMode(mode);
    applyMode(mode);
  }
</script>

<button
  type="button"
  class="ghost icon-button theme-toggle"
  onclick={cycle}
  {title}
  aria-label={`${title}. Click to change theme.`}
>
  {#if mode === 'light'}
    <svg class="theme-toggle-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <circle cx="12" cy="12" r="4.25" stroke="currentColor" stroke-width="1.6" />
      <path
        d="M12 2.5v2.4M12 19.1v2.4M21.5 12h-2.4M4.9 12H2.5M18.7 5.3l-1.7 1.7M7 17l-1.7 1.7M18.7 18.7 17 17M7 7 5.3 5.3"
        stroke="currentColor"
        stroke-width="1.6"
        stroke-linecap="round"
      />
    </svg>
  {:else if mode === 'dark'}
    <svg class="theme-toggle-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <path
        d="M20 14.2A8 8 0 1 1 9.8 4a6.4 6.4 0 0 0 10.2 10.2Z"
        stroke="currentColor"
        stroke-width="1.6"
        stroke-linejoin="round"
      />
    </svg>
  {:else}
    <svg class="theme-toggle-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <rect x="3" y="4.5" width="18" height="12" rx="2" stroke="currentColor" stroke-width="1.6" />
      <path d="M9 20h6M12 16.5V20" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
    </svg>
  {/if}
</button>
