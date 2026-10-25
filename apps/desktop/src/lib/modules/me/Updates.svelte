<script lang="ts">
  // Profi → „Updates“: the version, the update channel and the last check (ADR-006, D1).
  // The check and the download are the shell's; a found update waits here and in the
  // banner for the click.
  import { getApp } from '../../app.svelte';
  import type { UpdateChannel } from '../../types/UpdateChannel';

  const app = getApp();
  const { updater } = app;
  const channels: { id: UpdateChannel; label: string; hint: string }[] = [
    { id: 'stable', label: 'Stabil', hint: 'Was die Gang schon einen Abend lang benutzt hat.' },
    { id: 'beta', label: 'Beta', hint: 'Neues etwas früher, für Mittester.' },
    { id: 'nightly', label: 'Nightly', hint: 'Jeder neue Stand. Kann kaputt sein.' },
  ];
  const current = $derived(app.appSettings?.updateChannel ?? 'stable');
  const browser = $derived(app.info !== null && app.info.protocolVersion === 0);
  const version = $derived(app.info === null ? '…' : browser ? 'Browser-Modus' : app.info.version);
  const checkedAt = $derived(
    updater.checkedAt?.toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' }),
  );
  const status = $derived.by(() => {
    if (updater.checking) return 'Suche nach Updates …';
    const s = updater.status;
    if (s === null) return 'Noch nicht geprüft.';
    switch (s.kind) {
      case 'upToDate':
        return `Auf dem neuesten Stand (geprüft ${checkedAt}).`;
      case 'ready':
        return `Update auf ${s.version} ist heruntergeladen und geprüft.`;
      case 'unavailable':
        return s.reason;
      case 'failed':
        return `Die Suche ist fehlgeschlagen: ${s.reason}`;
    }
  });

  /** A new channel: stored, then looked at right away. */
  async function choose(channel: UpdateChannel) {
    await app.updateAppSettings({ updateChannel: channel });
    await updater.check();
  }
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
          onchange={() => void choose(channel.id)}
        />
        <span>
          <strong>{channel.label}</strong>
          <span class="note">{channel.hint}</span>
        </span>
      </label>
    {/each}
  </fieldset>
  <div class="check">
    <p class="status" role="status">{status}</p>
    {#if updater.ready}
      <button onclick={() => void app.installUpdate()}>
        {app.session.inRoom ? 'Call verlassen und installieren' : 'Jetzt installieren'}
      </button>
    {:else}
      <button onclick={() => void updater.check()} disabled={updater.checking}>
        Jetzt nach Updates suchen
      </button>
    {/if}
  </div>
  {#if updater.ready?.notes}
    <details>
      <summary>Was ist neu in {updater.ready.version}</summary>
      <pre class="notes">{updater.ready.notes}</pre>
    </details>
  {/if}
  <p class="note">
    yAPPA schaut beim Start und dann stündlich nach; ein Update wird im Hintergrund geladen und erst
    auf Klick installiert – nie mitten im Call. Danach startet die App neu.
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

  .check {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    align-items: flex-start;
    margin-top: var(--space-2);
  }

  .notes {
    margin: var(--space-2) 0 0;
    white-space: pre-wrap;
    font-size: var(--font-size-sm);
  }

  .note {
    margin: 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    font-weight: normal;
  }
</style>
