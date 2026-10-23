<script lang="ts">
  // The microphone with a live level: device, input channel of the device, the bar. Shared by
  // „Audio und Sprechen“ and onboarding step 2.
  import { getApp } from '../../app.svelte';
  import Info from '../../Info.svelte';
  import Meter from '../../Meter.svelte';

  let { note = true }: { note?: boolean } = $props();

  const { session } = getApp();
  const deviceValue = (id: string | null | undefined) => id ?? '';
  const deviceId = (value: string) => (value === '' ? null : value);
  const settings = $derived(session.settings);
  const channels = $derived(session.stats?.capture?.channels ?? 1);
</script>

<div class="label"><label for="input">Mikrofon</label><Info term="pegel" /></div>
<select
  id="input"
  value={deviceValue(settings?.inputDevice)}
  onchange={(e) => session.setInput(deviceId(e.currentTarget.value))}
>
  <option value="">Standardgerät</option>
  {#each session.inputs as device (device.id)}
    <option value={device.id}>{device.name}</option>
  {/each}
</select>
{#if channels > 1}
  <label for="channel">Eingang am Gerät</label>
  <select
    id="channel"
    value={String(settings?.inputChannel ?? 0)}
    onchange={(e) => session.applySettings({ inputChannel: Number(e.currentTarget.value) })}
  >
    {#each Array.from({ length: channels }, (_, i) => i) as channel (channel)}
      <option value={String(channel)}>Eingang {channel + 1}</option>
    {/each}
  </select>
{/if}
<Meter db={session.inputDb} active={session.sending} label="Mikrofontest" />
{#if note}
  <p class="note">
    {session.muted
      ? 'Mikrofon ist stummgeschaltet.'
      : session.sending
        ? 'Du gehst gerade raus – der Balken zeigt, was ankommt.'
        : 'Das Mikro hört, sendet aber gerade nicht.'}
  </p>
{/if}

<style>
  .label {
    display: flex;
    align-items: center;
    margin-top: var(--space-3);
  }

  label {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  label[for='channel'] {
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
</style>
