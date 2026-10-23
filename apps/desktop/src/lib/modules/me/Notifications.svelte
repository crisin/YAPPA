<script lang="ts">
  // „Benachrichtigungen“: one switch for now. The app shows no system notifications yet; the
  // choice is stored so it holds once they come.
  import { getApp } from '../../app.svelte';

  const app = getApp();
  const on = $derived(app.appSettings?.notifications ?? true);
</script>

<section class="notifications" aria-labelledby="notifications-title">
  <h1 id="notifications-title">Benachrichtigungen</h1>
  <label>
    <input
      type="checkbox"
      checked={on}
      onchange={(e) => app.updateAppSettings({ notifications: e.currentTarget.checked })}
    />
    Hinweise vom System zeigen, wenn yAPPA im Hintergrund ist
  </label>
  <p class="note">
    Noch zeigt yAPPA keine solchen Hinweise – die Einstellung gilt, sobald es sie gibt.
  </p>
</section>

<style>
  .notifications {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    max-width: 480px;
  }

  h1 {
    margin: 0 0 var(--space-2);
    font-size: var(--font-size-lg);
  }

  label {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  input {
    accent-color: var(--color-accent);
  }

  .note {
    margin: 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }
</style>
