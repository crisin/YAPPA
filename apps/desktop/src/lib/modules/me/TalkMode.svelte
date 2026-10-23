<script lang="ts">
  // „Wann geht das Mikro an?“: the speaking mode in plain words, with the threshold for
  // „Automatisch“ and the key for „Taste halten“. Shared by „Audio und Sprechen“ and
  // onboarding step 3.
  import { getApp } from '../../app.svelte';
  import Meter from '../../Meter.svelte';
  import PttKey from './PttKey.svelte';
  import type { TransmitMode } from '../../types/TransmitMode';

  const app = getApp();
  const { session } = app;

  const modes: { id: TransmitMode; label: string; hint: string }[] = [
    {
      id: 'voiceActivation',
      label: 'Automatisch',
      hint: 'Das Mikro geht an, wenn du sprichst. Empfohlen mit Headset.',
    },
    { id: 'pushToTalk', label: 'Taste halten', hint: 'Nur solange du eine Taste hältst.' },
    { id: 'alwaysOn', label: 'Immer an', hint: 'Alles, was das Mikro hört, geht raus.' },
  ];

  const settings = $derived(session.settings);
  // The gate decides on the level after the noise suppression — show that one.
  const gateInput = $derived(session.stages.find((s) => s.id === 'denoiser')?.levelDb ?? -120);
  const threshold = $derived(settings?.vadThresholdDb ?? -50);
</script>

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
  <label for="threshold">Ab hier geht das Mikro an: {Math.round(threshold)} dB</label>
  <input
    id="threshold"
    type="range"
    min="-80"
    max="-20"
    step="1"
    value={threshold}
    onchange={(e) => session.applySettings({ vadThresholdDb: Number(e.currentTarget.value) })}
  />
  <Meter db={gateInput} active={gateInput > threshold} label="Pegel nach der Rauschunterdrückung" />
  <p class="note">
    Klingst du abgehackt, schieb den Regler nach links. Gehen Tastatur oder Lüfter durch, nach
    rechts.
  </p>
{:else if settings?.transmitMode === 'pushToTalk'}
  <PttKey />
{/if}

<style>
  fieldset {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin-top: var(--space-4);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: var(--space-3) var(--space-4);
  }

  legend,
  label[for] {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  label[for] {
    margin-top: var(--space-3);
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

  input {
    accent-color: var(--color-accent);
  }

  .note {
    margin: 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    font-weight: normal;
  }
</style>
