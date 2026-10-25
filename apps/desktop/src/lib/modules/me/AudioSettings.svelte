<script lang="ts">
  // „Audio und Sprechen“: devices, a live microphone test, how the microphone goes on air,
  // noise suppression and volume. Plain words here; the terms are behind the ⓘ and in the
  // pro view. Microphone and speaking mode are parts the onboarding (U4) shows too.
  import { getApp } from '../../app.svelte';
  import Info from '../../Info.svelte';
  import MicTest from './MicTest.svelte';
  import QualityPicker from './QualityPicker.svelte';
  import TalkMode from './TalkMode.svelte';
  import { Session } from '../../session.svelte';
  import type { DenoiserKind } from '../../types/DenoiserKind';

  const app = getApp();
  const { session } = app;
  const deviceValue = (id: string | null | undefined) => id ?? '';
  const deviceId = (value: string) => (value === '' ? null : value);

  const denoisers: { id: DenoiserKind; label: string }[] = [
    { id: 'deepFilter', label: 'Stark (DeepFilterNet)' },
    { id: 'rnnoise', label: 'Leicht (RNNoise, wenig CPU)' },
    { id: 'off', label: 'Aus' },
  ];

  const settings = $derived(session.settings);
  const outputAway = $derived(Session.fallbackNote('Ausgabe', session.outputState));
</script>

<section class="audio" aria-labelledby="audio-title">
  <h1 id="audio-title">Audio und Sprechen</h1>

  <MicTest />
  <TalkMode />

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

  <QualityPicker />
  {#if app.view === 'pro'}
    <button class="quiet tight" onclick={() => app.openMe('netzwerk')}>
      Bitrate, Stereo, RED und DTX einzeln in „Netzwerk“
    </button>
  {/if}

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
  {#if outputAway}
    <p class="away" role="status">{outputAway}</p>
  {/if}
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
  {#if app.view === 'pro'}
    <label class="check">
      <input
        type="checkbox"
        checked={settings?.autoLevel ?? true}
        onchange={(e) => session.applySettings({ autoLevel: e.currentTarget.checked })}
      />
      Lautstärke der anderen automatisch angleichen
      <Info term="lautstaerke-angleich" />
    </label>
  {/if}

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

  label {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
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

  input[type='range'] {
    accent-color: var(--color-accent);
  }

  .check {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-top: var(--space-2);
    color: var(--color-text);
  }

  .tight {
    margin-top: 0;
  }

  .away {
    margin: 0;
    color: var(--color-warning);
    font-size: var(--font-size-sm);
  }

  .quiet {
    align-self: flex-start;
    margin-top: var(--space-4);
    background: transparent;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }
</style>
