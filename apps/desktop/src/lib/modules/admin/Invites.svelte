<script lang="ts">
  // Admin → „Einladungen“: create a link for one person (7 days, works once), copy it, and
  // see which links are still open, used or expired. Open ones can be withdrawn.
  import { getApp } from '../../app.svelte';
  import { since } from './time';

  const { admin } = getApp();
  let copied = $state(false);

  const now = () => Date.now();
  const stateOf = (usedBy: string | null, expiresAt: string) =>
    usedBy ? 'benutzt' : new Date(expiresAt).getTime() < now() ? 'abgelaufen' : 'offen';

  async function copy() {
    if (!admin.newLink) return;
    try {
      await navigator.clipboard.writeText(admin.newLink);
      copied = true;
      setTimeout(() => (copied = false), 2000);
    } catch {
      // No clipboard (browser without permission): the link stays selectable below.
    }
  }

  const sorted = $derived(
    [...admin.invites].sort((a, b) => b.createdAt.localeCompare(a.createdAt)),
  );
</script>

<section class="invites" aria-labelledby="invites-title">
  <h1 id="invites-title">Einladungen</h1>
  <div class="create">
    <button class="primary" disabled={admin.busy} onclick={() => admin.createInvite()}>
      Neue Einladung
    </button>
    <p class="note">Ein Link pro Person. Er gilt 7 Tage und funktioniert genau einmal.</p>
  </div>

  {#if admin.newLink}
    <div class="link" role="status">
      <label for="new-link">Neuer Link – schick ihn zusammen mit dem Installer</label>
      <div class="row">
        <input
          id="new-link"
          readonly
          value={admin.newLink}
          onfocus={(e) => e.currentTarget.select()}
        />
        <button onclick={copy}>{copied ? 'Kopiert' : 'Kopieren'}</button>
      </div>
    </div>
  {/if}

  <h2>Alle Einladungen</h2>
  <ul class="list">
    {#each sorted as invite (invite.code)}
      {@const state = stateOf(invite.usedBy, invite.expiresAt)}
      <li>
        <code>{invite.code}</code>
        <span class="state" class:open={state === 'offen'}>{state}</span>
        <span class="muted">
          erstellt {since(invite.createdAt)}{state === 'offen'
            ? ` · läuft ab ${since(invite.expiresAt)}`
            : ''}
        </span>
        {#if state === 'offen'}
          <button
            class="quiet"
            disabled={admin.busy}
            onclick={() => admin.revokeInvite(invite.code)}
            aria-label="Einladung {invite.code} zurückziehen"
          >
            Zurückziehen
          </button>
        {/if}
      </li>
    {:else}
      <li class="muted">{admin.busy ? 'Lade …' : 'Noch keine Einladungen.'}</li>
    {/each}
  </ul>
</section>

<style>
  .invites {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    max-width: 640px;
  }

  h1 {
    margin: 0;
    font-size: var(--font-size-lg);
  }

  h2 {
    margin: var(--space-3) 0 0;
    font-size: var(--font-size-md);
  }

  .create {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
  }

  .primary {
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: var(--color-accent-text);
    font-weight: 600;
  }

  .link {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .row {
    display: flex;
    gap: var(--space-2);
  }

  label {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  input {
    flex: 1;
    font-family: var(--font-mono);
    font-size: var(--font-size-sm);
    color: var(--color-text);
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: var(--space-2);
    user-select: text;
  }

  .list {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .list li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) 0;
    border-bottom: 1px solid var(--color-border);
    font-size: var(--font-size-sm);
  }

  code {
    font-family: var(--font-mono);
  }

  .state {
    color: var(--color-text-muted);
  }

  .state.open {
    color: var(--color-accent);
    font-weight: 600;
  }

  .muted,
  .note {
    margin: 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .quiet {
    margin-left: auto;
    background: transparent;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }
</style>
