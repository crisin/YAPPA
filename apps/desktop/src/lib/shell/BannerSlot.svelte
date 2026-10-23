<script lang="ts">
  // Zone F: one slot for whatever concerns the whole app. At most one banner, the most
  // important one; the queue lives in the shell.
  import { X } from '@lucide/svelte';
  import { getApp } from '../app.svelte';
  import { BANNER } from '../shell.svelte';

  const { shell } = getApp();
  const banner = $derived(shell.banner);
  // Errors and blocks interrupt; the rest is a polite status.
  const urgent = $derived(banner !== undefined && banner.priority <= BANNER.error);
</script>

{#if banner}
  <div class="banner" class:urgent role={urgent ? 'alert' : 'status'}>
    <span class="text">{banner.text}</span>
    {#if banner.action}
      <button onclick={() => banner.action?.run()}>{banner.action.label}</button>
    {/if}
    {#if banner.dismissible}
      <button
        class="close"
        aria-label="Hinweis schließen"
        onclick={() => shell.dismissBanner(banner.id)}
      >
        <X size={16} aria-hidden="true" />
      </button>
    {/if}
  </div>
{/if}

<style>
  .banner {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin: var(--space-3) var(--space-4) 0;
    padding: var(--space-2) var(--space-3);
    border: 1px solid var(--color-warning);
    border-radius: var(--radius-md);
    background: var(--color-surface);
    user-select: text;
  }

  .banner.urgent {
    border-color: var(--color-danger);
  }

  .text {
    flex: 1;
  }

  .close {
    display: grid;
    place-items: center;
    padding: var(--space-1);
    background: transparent;
    border-color: transparent;
  }
</style>
