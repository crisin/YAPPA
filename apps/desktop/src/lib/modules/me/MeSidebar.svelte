<script lang="ts">
  // Zone B of „Ich“: the settings navigation. U3 adds the Einfach/Profi switch on top.
  import { getApp, ME_SECTIONS } from '../../app.svelte';

  const app = getApp();
  const version = $derived(
    app.info === null
      ? '…'
      : app.info.protocolVersion === 0
        ? 'Browser-Modus'
        : `v${app.info.version}`,
  );
</script>

<nav class="sidebar" aria-label="Einstellungen">
  <h2>Ich</h2>
  <ul>
    {#each ME_SECTIONS as section (section.id)}
      <li>
        <button
          aria-current={app.meSection === section.id ? 'page' : undefined}
          onclick={() => (app.meSection = section.id)}
        >
          {section.label}
        </button>
      </li>
    {/each}
  </ul>
  <p class="meta">{version}</p>
</nav>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: var(--space-3);
  }

  h2 {
    margin: 0 0 var(--space-4);
    padding: var(--space-1) var(--space-2);
    font-size: var(--font-size-lg);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  button {
    width: 100%;
    text-align: left;
    background: transparent;
    border-color: transparent;
  }

  button[aria-current='page'] {
    background: var(--color-surface-raised);
    font-weight: 600;
  }

  .meta {
    margin: auto var(--space-2) 0;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    font-size: var(--font-size-sm);
  }
</style>
