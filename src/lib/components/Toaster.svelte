<script lang="ts">
  import { dismissToast, getToasts } from '$lib/stores/toasts.svelte';

  const toasts = $derived(getToasts());
</script>

<div class="toaster" aria-live="polite" aria-atomic="false">
  {#each toasts as toast (toast.id)}
    <div class={`toast toast-${toast.type}`} role="status">
      <span class="toast-message">{toast.message}</span>
      <button
        type="button"
        class="toast-close"
        onclick={() => dismissToast(toast.id)}
        aria-label="Dismiss notification"
      >×</button>
    </div>
  {/each}
</div>
