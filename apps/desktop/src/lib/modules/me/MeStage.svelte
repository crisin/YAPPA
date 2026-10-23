<script lang="ts">
  // Zone C of „Ich“: the chosen section. The call goes on meanwhile — the dock stays.
  import { getApp } from '../../app.svelte';
  import DebugPanel from '../../DebugPanel.svelte';
  import Help from '../../Help.svelte';
  import Appearance from './Appearance.svelte';
  import AudioSettings from './AudioSettings.svelte';

  const app = getApp();
</script>

{#if app.meSection === 'audio'}
  <AudioSettings />
{:else if app.meSection === 'darstellung'}
  <Appearance />
{:else if app.meSection === 'diagnose'}
  <DebugPanel session={app.session} info={app.info} />
{:else}
  <Help />
{/if}
