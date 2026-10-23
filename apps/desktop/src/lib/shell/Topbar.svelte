<script lang="ts">
  // Zone E: where am I (breadcrumb) and the way to anywhere (`Strg K`).
  import { Search } from '@lucide/svelte';
  import { getApp } from '../app.svelte';

  const { shell } = getApp();
  const module = $derived(shell.activeModule);
  const crumb = $derived(module?.crumb?.() ?? null);
  const mac = /Mac|iPhone|iPad/.test(globalThis.navigator?.platform ?? '');
</script>

<header class="topbar">
  <nav aria-label="Pfad" class="crumbs">
    <span class="module">{module?.label ?? ''}</span>
    {#if crumb}
      <span class="sep" aria-hidden="true">/</span>
      <span class="crumb">{crumb}</span>
    {/if}
  </nav>
  <button class="search" onclick={() => shell.openPalette()} aria-keyshortcuts="Control+K Meta+K">
    <Search size={16} aria-hidden="true" />
    <span>Suchen oder springen …</span>
    <kbd>{mac ? '⌘' : 'Strg'} K</kbd>
  </button>
</header>

<style>
  .topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    height: var(--topbar-height);
    padding: 0 var(--space-4);
    border-bottom: 1px solid var(--color-border);
  }

  .crumbs {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    min-width: 0;
    white-space: nowrap;
  }

  .module {
    font-weight: 600;
  }

  .sep,
  .crumb {
    color: var(--color-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .search {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 240px;
    color: var(--color-text-muted);
    background: var(--color-surface);
  }

  .search span {
    flex: 1;
    text-align: left;
  }

  kbd {
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    padding: 0 var(--space-1);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
  }
</style>
