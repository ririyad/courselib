<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    open = false,
    title,
    message = '',
    confirmLabel = 'Confirm',
    cancelLabel = 'Cancel',
    tone = 'default',
    busy = false,
    onConfirm,
    onCancel,
    children
  }: {
    open?: boolean;
    title: string;
    message?: string;
    confirmLabel?: string;
    cancelLabel?: string;
    tone?: 'default' | 'danger';
    busy?: boolean;
    onConfirm: () => void;
    onCancel: () => void;
    children?: Snippet;
  } = $props();

  let dialogEl = $state<HTMLDivElement | null>(null);
  let confirmEl = $state<HTMLButtonElement | null>(null);

  $effect(() => {
    if (open) {
      queueMicrotask(() => confirmEl?.focus());
    }
  });

  function requestCancel() {
    if (!busy) onCancel();
  }

  function onKeydown(event: KeyboardEvent) {
    if (!open) return;

    if (event.key === 'Escape') {
      event.preventDefault();
      requestCancel();
      return;
    }

    if (event.key === 'Tab' && dialogEl) {
      const focusable = dialogEl.querySelectorAll<HTMLElement>(
        'button:not([disabled]), [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
      );
      if (focusable.length === 0) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if open}
  <div class="dialog-overlay" role="presentation">
    <button
      type="button"
      class="dialog-backdrop"
      aria-label="Cancel"
      tabindex="-1"
      onclick={requestCancel}
    ></button>
    <div
      class="dialog"
      bind:this={dialogEl}
      role="dialog"
      aria-modal="true"
      aria-labelledby="dialog-title"
    >
      <h2 id="dialog-title" class="dialog-title">{title}</h2>
      {#if message}
        <p class="dialog-message">{message}</p>
      {/if}
      {#if children}
        <div class="dialog-body">{@render children()}</div>
      {/if}
      <div class="dialog-actions">
        <button type="button" class="ghost" onclick={requestCancel} disabled={busy}>
          {cancelLabel}
        </button>
        <button
          type="button"
          class={tone === 'danger' ? 'danger' : ''}
          class:busy
          bind:this={confirmEl}
          onclick={onConfirm}
          disabled={busy}
        >
          {confirmLabel}
        </button>
      </div>
    </div>
  </div>
{/if}
