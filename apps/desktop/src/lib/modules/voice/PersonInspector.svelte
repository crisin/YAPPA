<script lang="ts">
  // Inspector "person": how loud this one is for me, or not at all. Only I hear the change.
  import { getApp } from '../../app.svelte';
  import Meter from '../../Meter.svelte';

  const { session, shell } = getApp();
  const id = $derived(
    shell.inspector.content?.kind === 'person' ? shell.inspector.content.id : null,
  );
  const person = $derived(session.participants.find((p) => p.identity === id) ?? null);
  const gain = $derived(id ? (session.peerGainDb[id] ?? 0) : 0);
  const localMuted = $derived(id ? (session.peerMuted[id] ?? false) : false);
</script>

{#if person}
  <h2>{person.name}</h2>
  <p class="state">
    {person.muted
      ? 'hat sich stummgeschaltet'
      : session.peerLevels[person.identity]?.speaking
        ? 'spricht'
        : 'still'}
  </p>
  <Meter
    db={session.peerLevels[person.identity]?.levelDb ?? -120}
    active={session.peerLevels[person.identity]?.speaking ?? false}
    label="Pegel {person.name} im Detail"
  />
  <label for="inspector-gain">Lautstärke für mich</label>
  <div class="row">
    <input
      id="inspector-gain"
      type="range"
      min="-30"
      max="12"
      step="1"
      value={gain}
      oninput={(e) => session.setPeerGain(person.identity, Number(e.currentTarget.value))}
    />
    <span class="db">{gain > 0 ? '+' : ''}{gain} dB</span>
  </div>
  <button aria-pressed={localMuted} onclick={() => session.togglePeerMute(person.identity)}>
    {localMuted ? 'Für mich stumm' : 'Für mich stummschalten'}
  </button>
  <p class="hint">Nur du hörst die Änderung.</p>
{:else}
  <p class="hint">Diese Person ist nicht in deinem Channel.</p>
{/if}

<style>
  h2 {
    margin: 0 var(--space-6) var(--space-1) 0;
    font-size: var(--font-size-lg);
  }

  .state,
  .hint,
  label {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .state {
    margin: 0 0 var(--space-3);
  }

  label {
    display: block;
    margin-top: var(--space-4);
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-bottom: var(--space-3);
  }

  input {
    flex: 1;
    accent-color: var(--color-accent);
  }

  .db {
    font-family: var(--font-mono);
    font-size: var(--font-size-sm);
  }
</style>
