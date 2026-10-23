<script lang="ts">
  // „Darstellung“: the theme. A theme is a file in ui-tokens/, never code.
  import { getApp, type Theme } from '../../app.svelte';

  const app = getApp();
  const themes: { id: Theme; label: string }[] = [
    { id: 'dark', label: 'Dunkel' },
    { id: 'light', label: 'Hell' },
  ];
</script>

<section aria-labelledby="appearance-title">
  <h1 id="appearance-title">Darstellung</h1>
  <fieldset>
    <legend>Farbschema</legend>
    {#each themes as theme (theme.id)}
      <label>
        <input
          type="radio"
          name="theme"
          value={theme.id}
          checked={app.theme === theme.id}
          onchange={() => app.setTheme(theme.id)}
        />
        {theme.label}
      </label>
    {/each}
  </fieldset>
</section>

<style>
  h1 {
    margin: 0 0 var(--space-4);
    font-size: var(--font-size-lg);
  }

  fieldset {
    display: flex;
    gap: var(--space-4);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: var(--space-3) var(--space-4);
  }

  legend {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  label {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  input {
    accent-color: var(--color-accent);
  }
</style>
