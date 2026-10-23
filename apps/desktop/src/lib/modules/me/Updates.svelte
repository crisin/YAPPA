<script lang="ts">
  // Profi → „Updates“: the version and the update channel (ADR-006). The updater comes with
  // D1; until then the channel is only stored.
  import { getApp } from '../../app.svelte';
  import type { UpdateChannel } from '../../types/UpdateChannel';

  const app = getApp();
  const channels: { id: UpdateChannel; label: string; hint: string }[] = [
    { id: 'stable', label: 'Stabil', hint: 'Was die Gang schon einen Abend lang benutzt hat.' },
    { id: 'beta', label: 'Beta', hint: 'Neues etwas früher, für Mittester.' },
    { id: 'nightly', label: 'Nightly', hint: 'Jeder neue Stand. Kann kaputt sein.' },
  ];
  const current = $derived(app.appSettings?.updateChannel ?? 'stable');
  const version = $derived(
    app.info === null ? '…' : app.info.protocolVersion === 0 ? 'Browser-Modus' : app.info.version,
  );
</script>

<section class="updates" aria-labelledby="updates-title">
  <h1 id="updates-title">Updates</h1>
  <p>Installiert: <strong>{version}</strong></p>
  <fieldset>
    <legend>Update-Kanal</legend>
    {#each channels as channel (channel.id)}
      <label class="choice">
        <input
          type="radio"
          name="channel"
          value={channel.id}
          checked={current === channel.id}
          onchange={() => app.updateAppSettings({ updateChannel: channel.id })}
        />
        <span>
          <strong>{channel.label}</strong>
          <span class="note">{channel.hint}</span>
        </span>
      </label>
    {/each}
  </fieldset>
  <p class="note">
    Automatische Updates kommen mit einer späteren Version. Bis dahin gibt es neue Versionen als
    Installer vom Admin.
  </p>
</section>

<style>
  .updates {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    max-width: 480px;
  }

  h1 {
    margin: 0 0 var(--space-2);
    font-size: var(--font-size-lg);
  }

  p {
    margin: 0;
  }

  fieldset {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin-top: var(--space-2);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: var(--space-3) var(--space-4);
  }

  legend {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .choice {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
  }

  .choice span {
    display: flex;
    flex-direction: column;
  }

  input {
    accent-color: var(--color-accent);
  }

  .note {
    margin: 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    font-weight: normal;
  }
</style>
