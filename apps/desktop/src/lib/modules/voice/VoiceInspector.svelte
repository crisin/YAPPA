<script lang="ts">
  // The voice module's inspector (zone G): a person, the soundboard (ADR-010) or the app
  // audio (ADR-011). The shell knows one inspector component per module; this one picks by
  // what was opened.
  import { getApp } from '../../app.svelte';
  import AppAudio from './AppAudio.svelte';
  import PersonInspector from './PersonInspector.svelte';
  import Soundboard from './Soundboard.svelte';

  const { shell } = getApp();
  const kind = $derived(shell.inspector.content?.kind ?? null);
</script>

{#if kind === 'soundboard'}
  <Soundboard />
{:else if kind === 'appaudio'}
  <AppAudio />
{:else}
  <PersonInspector />
{/if}
