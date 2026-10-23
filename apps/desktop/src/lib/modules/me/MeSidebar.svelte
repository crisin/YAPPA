<script lang="ts">
  // Zone B of „Ich“: the settings navigation, with the Einfach/Profi switch on top. Profi adds
  // sections; it never hides one of Einfach.
  import { getApp, ME_SECTIONS } from '../../app.svelte';
  import type { SettingsView } from '../../types/SettingsView';

  const app = getApp();
  const views: { id: SettingsView; label: string }[] = [
    { id: 'simple', label: 'Einfach' },
    { id: 'pro', label: 'Profi' },
  ];
  const sections = $derived(ME_SECTIONS.filter((s) => app.view === 'pro' || !s.pro));
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
  <div class="views" role="radiogroup" aria-label="Ansicht">
    {#each views as view (view.id)}
      <label class:on={app.view === view.id}>
        <input
          type="radio"
          name="view"
          value={view.id}
          checked={app.view === view.id}
          onchange={() => app.setView(view.id)}
        />
        {view.label}
      </label>
    {/each}
  </div>
  <ul>
    {#each sections as section (section.id)}
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
    margin: 0 0 var(--space-3);
    padding: var(--space-1) var(--space-2);
    font-size: var(--font-size-lg);
  }

  .views {
    display: flex;
    margin: 0 0 var(--space-4);
    padding: 2px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
  }

  .views label {
    position: relative;
    flex: 1;
    padding: var(--space-1) var(--space-2);
    border-radius: var(--radius-sm);
    text-align: center;
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    cursor: pointer;
  }

  .views label.on {
    background: var(--color-surface-raised);
    color: var(--color-text);
    font-weight: 600;
  }

  .views label:has(input:focus-visible) {
    outline: 2px solid var(--color-focus);
    outline-offset: 1px;
  }

  .views input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
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
