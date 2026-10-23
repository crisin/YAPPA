<script lang="ts">
  // Zone B of the voice module. Until the control plane exists there is one room — the one
  // the invite leads to — shown TeamSpeak-style with the people right under it. U2 turns
  // this into the channel tree.
  import { getApp } from '../../app.svelte';

  const { session, shell } = getApp();

  function personState(identity: string, muted: boolean, hasVoice: boolean): string {
    if (muted) return 'stumm';
    if (!hasVoice) return 'ohne Ton';
    return session.peerLevels[identity]?.speaking ? 'spricht' : '';
  }
</script>

<div class="sidebar">
  <h2 class="space">Die Gang</h2>
  <p class="heading">Voice-Channels</p>
  {#if session.room && session.connection !== 'disconnected'}
    <section class="channel" aria-label="Channel {session.room}">
      <div class="channel-name">
        <span>{session.room}</span>
        <span class="count">{session.participants.length + 1}</span>
      </div>
      <ul class="people" aria-label="Leute in {session.room}">
        <li><span class="who">{session.identity ?? 'Ich'} (du)</span></li>
        {#each session.participants as person (person.identity)}
          {@const state = personState(person.identity, person.muted, person.hasVoice)}
          <li>
            <button
              class="who"
              class:speaking={state === 'spricht'}
              onclick={() => shell.openInspector({ kind: 'person', id: person.identity })}
            >
              {person.name}
            </button>
            {#if state}<span class="state">{state}</span>{/if}
          </li>
        {/each}
      </ul>
    </section>
  {:else}
    <p class="empty">Noch in keinem Channel. Mit einer Einladung geht es los.</p>
  {/if}
</div>

<style>
  .sidebar {
    padding: var(--space-3);
  }

  .space {
    margin: 0 0 var(--space-4);
    padding: var(--space-1) var(--space-2);
    font-size: var(--font-size-lg);
  }

  .heading {
    margin: 0 var(--space-2) var(--space-2);
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .channel {
    padding: var(--space-2);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
  }

  .channel-name {
    display: flex;
    justify-content: space-between;
    font-weight: 600;
  }

  .count,
  .state,
  .empty {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    font-weight: normal;
  }

  .empty {
    margin: 0 var(--space-2);
  }

  .people {
    list-style: none;
    margin: var(--space-2) 0 0;
    padding: 0 0 0 var(--space-3);
  }

  .people li {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: var(--space-2);
    min-height: 28px;
  }

  .who {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  button.who {
    padding: 0 var(--space-1);
    background: transparent;
    border: 1px solid transparent;
    text-align: left;
  }

  button.who.speaking {
    border-color: var(--color-speaking);
  }
</style>
