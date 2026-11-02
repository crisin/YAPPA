<script lang="ts">
  // The frame every module lives in (ADR-004, concept zones A–G): rail | sidebar with the
  // voice dock | topbar, banner, stage and inspector. The shell renders the registry and
  // never names a module. The keyboard follows the shell's moves (review 2026-10-23): into
  // the module after a switch, into the inspector when it opens, back when it closes.
  import { tick, untrack } from 'svelte';
  import { getApp } from '../app.svelte';
  import { focusInto, refocus } from '../focus';
  import BannerSlot from './BannerSlot.svelte';
  import CommandPalette from './CommandPalette.svelte';
  import Inspector from './Inspector.svelte';
  import Rail from './Rail.svelte';
  import Topbar from './Topbar.svelte';
  import VoiceDock from './VoiceDock.svelte';

  const { shell } = getApp();
  const module = $derived(shell.activeModule);

  let context = $state<HTMLElement | null>(null);
  let stage = $state<HTMLElement | null>(null);
  let body = $state<HTMLElement | null>(null);

  /** What had the keyboard when a module was left, by module — `Esc` returns there if it is
   * still on the page (the rail, the dock); otherwise the module itself is the landing. */
  const leftAt = new Map<string, Element | null>();
  /** What had the keyboard when the inspector opened — it gets it back on close. */
  let opener: Element | null = null;
  let current = untrack(() => shell.active);
  let shown: string | null = null;

  type Landing = 'module' | 'back' | 'inspector' | 'opener';

  function land(where: Landing) {
    switch (where) {
      case 'inspector':
        // The panel itself, so the name „Details“ is read before its controls.
        refocus(body?.querySelector<HTMLElement>('#details'));
        break;
      case 'opener':
        if (!refocus(opener)) focusInto(context) ?? focusInto(stage);
        break;
      case 'back':
        if (!refocus(leftAt.get(shell.active))) focusInto(context) ?? focusInto(stage);
        break;
      case 'module':
        focusInto(context) ?? focusInto(stage);
        break;
    }
  }

  $effect(() => {
    const id = shell.active;
    const how = shell.navigation;
    const { open, content } = shell.inspector;
    untrack(() => {
      let where: Landing | null = null;
      if (id !== current) {
        leftAt.set(current, document.activeElement);
        current = id;
        where = how === 'back' ? 'back' : 'module';
      }
      const key = open && content ? `${content.kind}:${content.id}` : null;
      if (key !== shown) {
        if (key) {
          if (shown === null) opener = document.activeElement;
          where = 'inspector';
        } else if (where === null) {
          where = 'opener';
        }
        shown = key;
      }
      if (where) {
        const target = where;
        void tick().then(() => land(target));
      }
    });
  });
</script>

<div class="shell">
  <Rail />
  <div class="side">
    <div class="context" bind:this={context}>
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
    <div class="body" bind:this={body}>
      <main class="stage" aria-label={module?.label} bind:this={stage}>
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
