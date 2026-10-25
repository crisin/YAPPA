<script lang="ts">
  // Zone C of the voice module: the way in when nobody is logged in, the people of my
  // channel when I am in one, and a short hint in between.
  import { Music } from '@lucide/svelte';
  import { getApp } from '../../app.svelte';
  import LoginCard from '../../LoginCard.svelte';
  import Participants from '../../Participants.svelte';

  const { session, gang, shell } = getApp();
  const soundboardOpen = $derived(
    shell.inspector.open && shell.inspector.content?.kind === 'soundboard',
  );
  const inRoom = $derived(session.inRoom && session.connection !== 'connecting');
  const title = $derived(gang.channelName(session.room) ?? session.room ?? 'Raum');
</script>

{#if inRoom}
  <div class="head">
    <h1>{title}</h1>
    <button
      class="small"
      aria-pressed={soundboardOpen}
      onclick={() =>
        soundboardOpen
          ? shell.closeInspector()
          : shell.openInspector({ kind: 'soundboard', id: 'soundboard' })}
    >
      <Music size={14} aria-hidden="true" />
      Soundboard{session.playing.length > 0 ? ' · spielt' : ''}
    </button>
  </div>
  <Participants {session} />
{:else if gang.status === 'in'}
  <section class="hint" aria-labelledby="lobby-title">
    <h1 id="lobby-title">Hallo {gang.me?.displayName}</h1>
    {#if gang.problem}
      <p class="problem" role="alert">{gang.problem}</p>
    {/if}
    <p>Links siehst du, wer gerade wo ist. Ein Klick auf einen Channel, und du bist drin.</p>
    {#if session.connection === 'connecting'}<p>Verbinde …</p>{/if}
  </section>
{:else if gang.status === 'starting'}
  <p class="quiet">Verbinde mit der Gang …</p>
{:else}
  <LoginCard />
{/if}

<style>
  h1 {
    margin: 0 0 var(--space-4);
    font-size: var(--font-size-lg);
  }

  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-3);
  }

  .head button {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
  }

  .hint {
    max-width: 460px;
  }

  .hint p,
  .quiet {
    color: var(--color-text-muted);
  }

  .problem {
    color: var(--color-danger);
  }
</style>
