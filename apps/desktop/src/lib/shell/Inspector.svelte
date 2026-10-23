<script lang="ts">
  // Zone G: details and tools for a selection. Optional, never needed for a flow; closed by
  // default and remembered per module by the shell.
  import { X } from '@lucide/svelte';
  import { getApp } from '../app.svelte';

  const { shell } = getApp();
  const Content = $derived(shell.activeModule?.inspector);
</script>

{#if shell.inspector.open && Content}
  <aside class="inspector" aria-label="Details">
    <button class="close" aria-label="Details schließen" onclick={() => shell.closeInspector()}>
      <X size={16} aria-hidden="true" />
    </button>
    <Content />
  </aside>
{/if}

<style>
  .inspector {
    position: relative;
    width: var(--inspector-width);
    flex: none;
    overflow-y: auto;
    padding: var(--space-4);
    border-left: 1px solid var(--color-border);
    background: var(--color-surface);
  }

  .close {
    position: absolute;
    top: var(--space-3);
    right: var(--space-3);
    display: grid;
    place-items: center;
    padding: var(--space-1);
  }
</style>
