<script lang="ts">
  // The voice module's inspector (zone G): a person, or the soundboard (ADR-010). The shell
  // knows one inspector component per module; this one picks by what was opened.
  import { getApp } from '../../app.svelte';
  import PersonInspector from './PersonInspector.svelte';
  import Soundboard from './Soundboard.svelte';

  const { shell } = getApp();
  const kind = $derived(shell.inspector.content?.kind ?? null);
</script>

{#if kind === 'soundboard'}
  <Soundboard />
{:else}
  <PersonInspector />
{/if}
