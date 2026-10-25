<script lang="ts">
  // „Darstellung“: the theme (a file in ui-tokens/, never code) and the window — what the X
  // does and whether yAPPA starts with the login. Both are app settings; the shell does the
  // hiding and the OS entry.
  import { getApp, type Theme } from '../../app.svelte';
  import Info from '../../Info.svelte';
  import type { OnClose } from '../../types/OnClose';

  const app = getApp();
  const themes: { id: Theme; label: string }[] = [
    { id: 'dark', label: 'Dunkel' },
    { id: 'light', label: 'Hell' },
  ];
  const closes: { id: OnClose; label: string }[] = [
    { id: 'tray', label: 'In den Infobereich – der Call läuft weiter' },
    { id: 'quit', label: 'yAPPA beenden' },
  ];
  const onClose = $derived(app.appSettings?.onClose ?? 'tray');
  const autostart = $derived(app.appSettings?.autostart ?? false);
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
  <fieldset class="window">
    <legend>Fenster schließen (X) <Info term="infobereich" /></legend>
    {#each closes as close (close.id)}
      <label>
        <input
          type="radio"
          name="on-close"
          value={close.id}
          checked={onClose === close.id}
          onchange={() => app.updateAppSettings({ onClose: close.id })}
        />
        {close.label}
      </label>
    {/each}
    <p class="note">
      Im Infobereich bringt ein Klick auf das Katzen-Symbol das Fenster zurück; das Menü dort
      schaltet stumm oder taub und beendet yAPPA ganz.
    </p>
  </fieldset>
  <fieldset class="window">
    <legend>Start</legend>
    <label>
      <input
        type="checkbox"
        checked={autostart}
        onchange={(e) => app.updateAppSettings({ autostart: e.currentTarget.checked })}
      />
      yAPPA beim Anmelden am Rechner starten (klein im Infobereich)
    </label>
  </fieldset>
</section>

<style>
  h1 {
    margin: 0 0 var(--space-4);
    font-size: var(--font-size-lg);
  }

  section {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    max-width: 520px;
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

  .window {
    flex-direction: column;
    gap: var(--space-2);
  }

  .note {
    margin: 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }
</style>
