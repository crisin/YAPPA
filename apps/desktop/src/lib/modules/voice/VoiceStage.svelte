<script lang="ts">
  // Zone C of the voice module: the invite card outside a room, the people inside one.
  import { getApp } from '../../app.svelte';
  import JoinCard from '../../JoinCard.svelte';
  import Participants from '../../Participants.svelte';

  const { session } = getApp();
</script>

{#if session.inRoom && session.connection !== 'connecting'}
  <h1>{session.room ?? 'Raum'}</h1>
  <Participants {session} />
{:else}
  <JoinCard {session} />
{/if}

<style>
  h1 {
    margin: 0 0 var(--space-4);
    font-size: var(--font-size-lg);
  }
</style>
