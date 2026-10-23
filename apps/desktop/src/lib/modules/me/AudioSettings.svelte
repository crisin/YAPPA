<script lang="ts">
  // „Audio und Sprechen“: devices, a live microphone test, how the microphone goes on air,
  // noise suppression and volume. Plain words here; the terms are behind the ⓘ and in the
  // pro view. Built to be reused as step 2 of the onboarding (U4).
  import { getApp } from '../../app.svelte';
  import Info from '../../Info.svelte';
  import Meter from '../../Meter.svelte';
  import PttKey from './PttKey.svelte';
  import type { DenoiserKind } from '../../types/DenoiserKind';
  import type { TransmitMode } from '../../types/TransmitMode';

  const app = getApp();
  const { session } = app;
  const deviceValue = (id: string | null | undefined) => id ?? '';
  const deviceId = (value: string) => (value === '' ? null : value);

  const modes: { id: TransmitMode; label: string; hint: string }[] = [
    {
      id: 'voiceActivation',
      label: 'Automatisch',
      hint: 'Das Mikro geht an, wenn du sprichst. Empfohlen.',
    },
    { id: 'pushToTalk', label: 'Taste halten', hint: 'Nur solange du eine Taste hältst.' },
    { id: 'alwaysOn', label: 'Immer an', hint: 'Alles, was das Mikro hört, geht raus.' },
  ];
  const denoisers: { id: DenoiserKind; label: string }[] = [
    { id: 'deepFilter', label: 'Stark (DeepFilterNet)' },
    { id: 'rnnoise', label: 'Leicht (RNNoise, wenig CPU)' },
    { id: 'off', label: 'Aus' },
  ];

  const settings = $derived(session.settings);
  const channels = $derived(session.stats?.capture?.channels ?? 1);
  // The gate decides on the level after the noise suppression — show that one.
  const gateInput = $derived(session.stages.find((s) => s.id === 'denoiser')?.levelDb ?? -120);
  const threshold = $derived(settings?.vadThresholdDb ?? -50);
</script>

<section class="audio" aria-labelledby="audio-title">
  <h1 id="audio-title">Audio und Sprechen</h1>

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
  <p class="note">
    {session.muted
      ? 'Mikrofon ist stummgeschaltet.'
      : session.sending
        ? 'Du gehst gerade raus – der Balken zeigt, was ankommt.'
        : 'Das Mikro hört, sendet aber gerade nicht.'}
  </p>

  <fieldset>
    <legend>Wann geht das Mikro an?</legend>
    {#each modes as mode (mode.id)}
      <label class="choice">
        <input
          type="radio"
          name="mode"
          value={mode.id}
          checked={settings?.transmitMode === mode.id}
          onchange={() => app.setTransmitMode(mode.id)}
        />
        <span>
          <strong>{mode.label}</strong>
          <span class="note">{mode.hint}</span>
        </span>
      </label>
    {/each}
  </fieldset>

  {#if settings?.transmitMode === 'voiceActivation'}
    <label for="threshold">
      Ab hier geht das Mikro an: {Math.round(threshold)} dB
    </label>
    <input
      id="threshold"
      type="range"
      min="-80"
      max="-20"
      step="1"
      value={threshold}
      onchange={(e) => session.applySettings({ vadThresholdDb: Number(e.currentTarget.value) })}
    />
    <Meter
      db={gateInput}
      active={gateInput > threshold}
      label="Pegel nach der Rauschunterdrückung"
    />
    <p class="note">
      Klingst du abgehackt, schieb den Regler nach links. Gehen Tastatur oder Lüfter durch, nach
      rechts.
    </p>
  {:else if settings?.transmitMode === 'pushToTalk'}
    <PttKey />
  {/if}

  <label for="denoiser">Rauschunterdrückung</label>
  <select
    id="denoiser"
    value={settings?.denoiser ?? 'deepFilter'}
    onchange={(e) => session.applySettings({ denoiser: e.currentTarget.value as DenoiserKind })}
  >
    {#each denoisers as d (d.id)}
      <option value={d.id}>{d.label}</option>
    {/each}
  </select>

  <label for="output">Ausgabe</label>
  <select
    id="output"
    value={deviceValue(settings?.outputDevice)}
    onchange={(e) => session.setOutput(deviceId(e.currentTarget.value))}
  >
    <option value="">Standardgerät</option>
    {#each session.outputs as device (device.id)}
      <option value={device.id}>{device.name}</option>
    {/each}
  </select>
  <label for="volume">Lautstärke aller anderen: {Math.round(settings?.outputGainDb ?? 0)} dB</label>
  <input
    id="volume"
    type="range"
    min="-30"
    max="12"
    step="1"
    value={settings?.outputGainDb ?? 0}
    onchange={(e) => session.applySettings({ outputGainDb: Number(e.currentTarget.value) })}
  />

  <button class="quiet" onclick={() => session.refreshDevices()}>Geräte neu einlesen</button>
  {#if app.view === 'pro'}
    <button class="quiet" onclick={() => app.openMe('signalkette')}>
      Hochpass, Gate, EQ und Kompressor in „Signalkette“
    </button>
  {/if}
</section>

<style>
  .audio {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    max-width: 480px;
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

  label,
  legend {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .label label {
    margin-top: 0;
  }

  label[for] {
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

  input[type='range'],
  input[type='radio'] {
    accent-color: var(--color-accent);
  }

  fieldset {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin-top: var(--space-4);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: var(--space-3) var(--space-4);
  }

  .choice {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
    margin: 0;
    color: var(--color-text);
    font-size: var(--font-size-md);
  }

  .choice span {
    display: flex;
    flex-direction: column;
  }

  .note {
    margin: 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    font-weight: normal;
  }

  .quiet {
    align-self: flex-start;
    margin-top: var(--space-4);
    background: transparent;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }
</style>
