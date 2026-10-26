<script lang="ts">
  // Zone B of the voice module: the channel tree, TeamSpeak-style — people right under their
  // channel, so everybody sees who is where without going anywhere. One click (or Enter)
  // joins; a click on a person opens them in the inspector. States are words, not only
  // colours. Without a login (a pasted developer invite) there is only the one room.
  import { Music } from '@lucide/svelte';
  import { getApp } from '../../app.svelte';
  import type { PresenceEntry } from '../../api';

  const { session, shell, gang } = getApp();

  const inMyRoom = (id: string) =>
    session.inRoom && session.participants.some((p) => p.identity === id);

  /** Same threshold the engine uses for the others (engine.rs `SPEAKING_DB`). */
  const SPEAKING_DB = -45;

  function speaking(person: PresenceEntry): boolean {
    if (person.userId === gang.me?.id)
      return session.inRoom && !session.muted && session.inputDb > SPEAKING_DB;
    return session.peerLevels[person.userId]?.speaking ?? false;
  }

  /** A sound plays on their soundboard track — or on mine (ADR-010). */
  function playsSound(person: PresenceEntry): boolean {
    if (person.userId === gang.me?.id) return session.inRoom && session.playing.length > 0;
    return session.peerPlaysSound(person.userId);
  }

  function stateText(person: PresenceEntry): string {
    // Mute and deafen come as presence (A3, ADR-005) so they show in every channel, not
    // only in mine; the room's own mute flag is the fallback for an older client.
    const inRoomMuted = session.participants.find((p) => p.identity === person.userId)?.muted;
    if (person.userId === gang.me?.id) {
      if (session.deafened) return 'taub';
      if (session.muted) return 'stumm';
    }
    if (person.state.deafened) return 'taub';
    if (person.state.muted || inRoomMuted) return 'stumm';
    if (person.state.away) return 'abwesend';
    if (person.state.talk === 'ptt') return 'PTT';
    return '';
  }

  function openPerson(id: string) {
    shell.openInspector({ kind: 'person', id });
  }
</script>

<div class="sidebar">
  <h2 class="space">Die Gang</h2>
  {#if gang.status === 'in'}
    <p class="heading" id="channels-heading">Voice-Channels</p>
    <ul class="channels" aria-labelledby="channels-heading">
      {#each gang.channels as channel (channel.id)}
        {@const people = gang.peopleIn(channel.id)}
        {@const mine = gang.myChannel === channel.id}
        <li class="channel" class:mine>
          <button
            class="channel-name"
            aria-current={mine ? 'true' : undefined}
            aria-label="{channel.name}, {people.length === 1
              ? '1 Person'
              : `${people.length} Leute`}{mine ? ', du bist hier' : ', beitreten'}"
            disabled={gang.joining !== null}
            onclick={() => gang.join(channel.id)}
          >
            <span class="name">{channel.name}</span>
            {#if gang.joining === channel.id}
              <span class="count">…</span>
            {:else if !mine}
              <span class="join" aria-hidden="true">Beitreten</span>
            {/if}
            <span class="count" aria-hidden="true">{people.length}</span>
          </button>
          {#if people.length > 0}
            <ul class="people" aria-label="Leute in {channel.name}">
              {#each people as person (person.userId)}
                {@const me = person.userId === gang.me?.id}
                {@const state = stateText(person)}
                <li>
                  {#if me || !inMyRoom(person.userId)}
                    <span class="who" class:speaking={speaking(person)}>
                      {person.displayName}{me ? ' (du)' : ''}
                    </span>
                  {:else}
                    <button
                      class="who"
                      class:speaking={speaking(person)}
                      onclick={() => openPerson(person.userId)}
                    >
                      {person.displayName}
                    </button>
                  {/if}
                  {#if playsSound(person)}
                    <span class="sound" role="img" aria-label="spielt einen Sound">
                      <Music size={12} aria-hidden="true" />
                    </span>
                  {/if}
                  {#if state}<span class="state">{state}</span>{/if}
                </li>
              {/each}
            </ul>
          {/if}
        </li>
      {/each}
    </ul>
    {#if gang.offline}
      <p class="empty">Server nicht erreichbar – Liste vom letzten Mal.</p>
    {/if}
  {:else if session.room && session.connection !== 'disconnected'}
    <p class="heading">Raum</p>
    <section class="channel mine" aria-label="Channel {session.room}">
      <div class="channel-name static">
        <span class="name">{session.room}</span>
        <span class="count">{session.participants.length + 1}</span>
      </div>
      <ul class="people" aria-label="Leute in {session.room}">
        <li><span class="who">{session.identity ?? 'Ich'} (du)</span></li>
        {#each session.participants as person (person.identity)}
          <li>
            <button class="who" onclick={() => openPerson(person.identity)}>{person.name}</button>
            {#if session.peerPlaysSound(person.identity)}
              <span class="sound" role="img" aria-label="spielt einen Sound">
                <Music size={12} aria-hidden="true" />
              </span>
            {/if}
            {#if person.muted}<span class="state">stumm</span>{/if}
          </li>
        {/each}
      </ul>
    </section>
  {:else if gang.status === 'starting'}
    <p class="empty">Verbinde …</p>
  {:else}
    <p class="empty">Noch nicht dabei. Mit einer Einladung geht es los.</p>
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

  .channels {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .channel {
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    padding: var(--space-1);
  }

  .channel.mine {
    border-color: var(--color-border);
    background: var(--color-surface-raised);
  }

  .channel-name {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    width: 100%;
    padding: var(--space-1) var(--space-2);
    background: transparent;
    border-color: transparent;
    font-weight: 600;
    text-align: left;
  }

  .channel-name:disabled {
    cursor: default;
  }

  .channel-name .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .join {
    visibility: hidden;
    padding: 0 var(--space-2);
    border-radius: var(--radius-sm);
    background: var(--color-accent);
    color: var(--color-accent-text);
    font-size: var(--font-size-xs);
    font-weight: 600;
  }

  .channel-name:hover .join,
  .channel-name:focus-visible .join {
    visibility: visible;
  }

  .count,
  .state,
  .empty {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    font-weight: normal;
  }

  .sound {
    display: inline-flex;
    color: var(--color-accent);
  }

  .empty {
    margin: var(--space-2);
  }

  .people {
    list-style: none;
    margin: 0;
    padding: 0 var(--space-2) var(--space-1) var(--space-5);
  }

  .people li {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: var(--space-2);
    min-height: 26px;
  }

  .who {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    padding: 0 var(--space-1);
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
  }

  button.who {
    background: transparent;
    text-align: left;
  }

  .who.speaking {
    border-color: var(--color-speaking);
  }
</style>
