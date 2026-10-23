<script lang="ts">
  // Zone C of „Ich“: the chosen section. The call goes on meanwhile — the dock stays.
  import { getApp } from '../../app.svelte';
  import DebugPanel from '../../DebugPanel.svelte';
  import Help from '../../Help.svelte';
  import Account from './Account.svelte';
  import Appearance from './Appearance.svelte';
  import AudioSettings from './AudioSettings.svelte';
  import Keys from './Keys.svelte';
  import Network from './Network.svelte';
  import Notifications from './Notifications.svelte';
  import SignalChain from './SignalChain.svelte';
  import Updates from './Updates.svelte';

  const app = getApp();
</script>

{#if app.meSection === 'konto'}
  <Account />
{:else if app.meSection === 'audio'}
  <AudioSettings />
{:else if app.meSection === 'darstellung'}
  <Appearance />
{:else if app.meSection === 'signalkette'}
  <SignalChain />
{:else if app.meSection === 'tasten'}
  <Keys />
{:else if app.meSection === 'netzwerk'}
  <Network />
{:else if app.meSection === 'benachrichtigungen'}
  <Notifications />
{:else if app.meSection === 'updates'}
  <Updates />
{:else if app.meSection === 'diagnose'}
  <DebugPanel session={app.session} info={app.info} />
{:else}
  <Help />
{/if}
