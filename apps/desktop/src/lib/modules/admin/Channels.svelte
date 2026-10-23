<script lang="ts">
  // Admin → „Channels“: the list in the order the channel tree shows it. Add at the end,
  // rename in place, move up and down, delete (the last one stays — the server says so).
  // People in a deleted channel drop out of it.
  import { getApp } from '../../app.svelte';

  const { admin, gang } = getApp();
  let name = $state('');
  let editing = $state<string | null>(null);
  let draft = $state('');
  let confirming = $state<string | null>(null);

  async function add(event: SubmitEvent) {
    event.preventDefault();
    const trimmed = name.trim();
    if (!trimmed) return;
    await admin.addChannel(trimmed);
    if (!admin.problem) name = '';
  }

  function edit(id: string, current: string) {
    editing = id;
    draft = current;
  }

  async function save(event: SubmitEvent, id: string) {
    event.preventDefault();
    const trimmed = draft.trim();
    if (trimmed) await admin.renameChannel(id, trimmed);
    editing = null;
  }

  async function remove(id: string) {
    if (confirming !== id) {
      confirming = id;
      return;
    }
    confirming = null;
    await admin.deleteChannel(id);
  }
</script>

<section class="channels" aria-labelledby="channels-title">
  <h1 id="channels-title">Channels</h1>
  <ol class="list" aria-label="Channel-Reihenfolge">
    {#each gang.channels as channel, i (channel.id)}
      {@const people = gang.peopleIn(channel.id).length}
      <li>
        {#if editing === channel.id}
          <form class="rename" onsubmit={(e) => save(e, channel.id)}>
            <input
              aria-label="Neuer Name für {channel.name}"
              maxlength="40"
              bind:value={draft}
              onkeydown={(e) => e.key === 'Escape' && (e.preventDefault(), (editing = null))}
            />
            <button type="submit">Speichern</button>
          </form>
        {:else}
          <span class="name">{channel.name}</span>
          <span class="muted">{people === 0 ? 'leer' : `${people} drin`}</span>
          <span class="tools">
            <button
              class="quiet"
              disabled={i === 0 || admin.busy}
              onclick={() => admin.moveChannel(channel.id, i - 1)}
              aria-label="{channel.name} nach oben"
            >
              ↑
            </button>
            <button
              class="quiet"
              disabled={i === gang.channels.length - 1 || admin.busy}
              onclick={() => admin.moveChannel(channel.id, i + 1)}
              aria-label="{channel.name} nach unten"
            >
              ↓
            </button>
            <button class="quiet" onclick={() => edit(channel.id, channel.name)}>
              Umbenennen
            </button>
            <button
              class="quiet"
              class:danger={confirming === channel.id}
              disabled={admin.busy}
              onclick={() => remove(channel.id)}
              onblur={() => (confirming = null)}
              aria-label={confirming === channel.id
                ? `${channel.name} wirklich löschen?`
                : `${channel.name} löschen`}
            >
              {confirming === channel.id ? 'Wirklich löschen?' : 'Löschen'}
            </button>
          </span>
        {/if}
      </li>
    {/each}
  </ol>

  <form class="add" onsubmit={add}>
    <label for="new-channel">Neuer Channel</label>
    <div class="row">
      <input id="new-channel" maxlength="40" placeholder="z. B. Raid-Abend" bind:value={name} />
      <button type="submit" disabled={!name.trim() || admin.busy}>Anlegen</button>
    </div>
  </form>
  <p class="note">Der erste Channel ist die Lobby: Dort landet man nach der Einrichtung.</p>
</section>

<style>
  .channels {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    max-width: 640px;
  }

  h1 {
    margin: 0;
    font-size: var(--font-size-lg);
  }

  .list {
    margin: 0;
    padding: 0 0 0 var(--space-4);
  }

  .list li {
    padding: var(--space-1) 0;
    border-bottom: 1px solid var(--color-border);
  }

  .list li,
  .rename,
  .row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .name {
    font-weight: 600;
  }

  .tools {
    display: flex;
    gap: var(--space-1);
    margin-left: auto;
  }

  label {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .add {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    margin-top: var(--space-2);
  }

  input {
    flex: 1;
    font: inherit;
    color: var(--color-text);
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: var(--space-1) var(--space-2);
  }

  .muted,
  .note {
    margin: 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .quiet {
    background: transparent;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .danger {
    color: var(--color-danger);
    border-color: var(--color-danger);
  }
</style>
