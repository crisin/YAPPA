<script lang="ts">
  // The frame every module lives in (ADR-004, concept zones A–G): rail | sidebar with the
  // voice dock | topbar, banner, stage and inspector. The shell renders the registry and
  // never names a module.
  import { getApp } from '../app.svelte';
  import BannerSlot from './BannerSlot.svelte';
  import CommandPalette from './CommandPalette.svelte';
  import Inspector from './Inspector.svelte';
  import Rail from './Rail.svelte';
  import Topbar from './Topbar.svelte';
  import VoiceDock from './VoiceDock.svelte';

  const { shell } = getApp();
  const module = $derived(shell.activeModule);
</script>

<div class="shell">
  <Rail />
  <div class="side">
    <div class="context">
      {#if module}
        {@const Sidebar = module.sidebar}
        <Sidebar />
      {/if}
    </div>
    <VoiceDock />
  </div>
  <div class="main">
    <Topbar />
    <BannerSlot />
    <div class="body">
      <main class="stage" aria-label={module?.label}>
        {#if module}
          {@const Stage = module.stage}
          <Stage />
        {/if}
      </main>
      <Inspector />
    </div>
  </div>
</div>
<CommandPalette />

<style>
  .shell {
    display: grid;
    grid-template-columns: var(--rail-width) var(--sidebar-width) 1fr;
    height: 100%;
  }

  .side {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--color-surface);
    border-right: 1px solid var(--color-border);
  }

  .context {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }

  .main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }

  .body {
    flex: 1;
    display: flex;
    min-height: 0;
  }

  .stage {
    flex: 1;
    min-width: 0;
    overflow: auto;
    padding: var(--space-6);
  }
</style>
