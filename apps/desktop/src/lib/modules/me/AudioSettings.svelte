<script lang="ts">
  // „Audio und Sprechen“: devices and a live microphone test. Built to be reused as step 2
  // of the onboarding (U4), so it takes nothing but the app context.
  import { getApp } from '../../app.svelte';
  import Info from '../../Info.svelte';
  import Meter from '../../Meter.svelte';

  const { session } = getApp();
  const deviceValue = (id: string | null | undefined) => id ?? '';
  const deviceId = (value: string) => (value === '' ? null : value);
</script>

<section class="audio" aria-labelledby="audio-title">
  <h1 id="audio-title">Audio und Sprechen</h1>

  <div class="label"><label for="input">Mikrofon</label><Info term="pegel" /></div>
  <select
    id="input"
    value={deviceValue(session.settings?.inputDevice)}
    onchange={(e) => session.setInput(deviceId(e.currentTarget.value))}
  >
    <option value="">Standardgerät</option>
    {#each session.inputs as device (device.id)}
      <option value={device.id}>{device.name}</option>
    {/each}
  </select>
  <Meter db={session.inputDb} active={session.sending} label="Mikrofontest" />
  <p class="note">
    {session.muted
      ? 'Mikrofon ist aus.'
      : session.sending
        ? 'Sprich – der Balken zeigt, was ankommt.'
        : 'Mikrofon ist an, aber still.'}
  </p>

  <label for="output">Ausgabe</label>
  <select
    id="output"
    value={deviceValue(session.settings?.outputDevice)}
    onchange={(e) => session.setOutput(deviceId(e.currentTarget.value))}
  >
    <option value="">Standardgerät</option>
    {#each session.outputs as device (device.id)}
      <option value={device.id}>{device.name}</option>
    {/each}
  </select>

  <button class="quiet" onclick={() => session.refreshDevices()}>Geräte neu einlesen</button>
</section>

<style>
  .audio {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    max-width: 460px;
  }

  h1 {
    margin: 0 0 var(--space-2);
    font-size: var(--font-size-lg);
  }

  .label {
    display: flex;
    align-items: center;
    margin-top: var(--space-3);
  }

  label {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .label label {
    margin-top: 0;
  }

  label[for='output'] {
    margin-top: var(--space-3);
  }

  select {
    font: inherit;
    color: inherit;
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: var(--space-2);
  }

  .note {
    margin: 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .quiet {
    align-self: flex-start;
    margin-top: var(--space-3);
    background: transparent;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }
</style>
