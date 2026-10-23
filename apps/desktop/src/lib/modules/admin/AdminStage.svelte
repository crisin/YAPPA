<script lang="ts">
  // Zone C of the admin module: the open section, with the last problem on top. Loads the
  // section when the module opens; the voice dock stays, a call goes on meanwhile.
  import { onMount } from 'svelte';
  import { getApp } from '../../app.svelte';
  import Channels from './Channels.svelte';
  import Invites from './Invites.svelte';
  import Operations from './Operations.svelte';
  import People from './People.svelte';

  const { admin } = getApp();

  onMount(() => void admin.refresh());
</script>

{#if admin.problem}
  <p class="problem" role="alert">{admin.problem}</p>
{/if}
{#if admin.section === 'leute'}
  <People />
{:else if admin.section === 'einladungen'}
  <Invites />
{:else if admin.section === 'channels'}
  <Channels />
{:else}
  <Operations />
{/if}

<style>
  .problem {
    margin: 0 0 var(--space-3);
    color: var(--color-danger);
  }
</style>
