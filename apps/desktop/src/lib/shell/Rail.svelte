<script lang="ts">
  // Zone A: one level of modules, icon plus label, never icon only. „Ich“ and role-bound
  // modules sit at the bottom.
  import { getApp } from '../app.svelte';
  import type { ModuleDef } from '../shell.svelte';
  import logo from '../../assets/logo.png';

  const { shell } = getApp();
</script>

{#snippet entry(module: ModuleDef)}
  {@const Icon = module.icon}
  <li>
    <button
      class="slot"
      aria-current={shell.active === module.id ? 'page' : undefined}
      onclick={() => shell.open(module.id)}
    >
      <span class="icon"><Icon size={22} aria-hidden="true" /></span>
      <span class="label">{module.label}</span>
    </button>
  </li>
{/snippet}

<nav class="rail" aria-label="Bereiche">
  <img class="logo" src={logo} alt="yAPPA" width="36" height="36" />
  <ul>
    {#each shell.railMain as module (module.id)}
      {@render entry(module)}
    {/each}
  </ul>
  <ul class="bottom">
    {#each shell.railBottom as module (module.id)}
      {@render entry(module)}
    {/each}
  </ul>
</nav>

<style>
  .rail {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-3);
    width: var(--rail-width);
    padding: var(--space-3) 0;
    background: var(--color-surface-sunken);
    border-right: 1px solid var(--color-border);
  }

  .logo {
    border-radius: var(--radius-md);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .bottom {
    margin-top: auto;
  }

  .slot {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-1);
    width: calc(var(--rail-width) - var(--space-3));
    padding: var(--space-2) 0;
    background: transparent;
    border-color: transparent;
    color: var(--color-text-muted);
    font-size: var(--font-size-xs);
  }

  .slot:hover {
    color: var(--color-text);
    border-color: transparent;
    background: var(--color-surface);
  }

  .slot[aria-current='page'] {
    color: var(--color-text);
    background: var(--color-surface-raised);
  }

  .icon {
    display: grid;
    place-items: center;
  }
</style>
