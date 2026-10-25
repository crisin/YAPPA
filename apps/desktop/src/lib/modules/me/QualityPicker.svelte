<script lang="ts">
  // The voice quality as a choice of profiles. Shared by „Audio und Sprechen“ (Einfach) and
  // „Netzwerk“ (Profi, where the values behind it are editable). A change during a call
  // publishes the voice track again — the others hear a short gap, nobody has to rejoin.
  import { getApp } from '../../app.svelte';
  import Info from '../../Info.svelte';
  import { PROFILES, PROFILE_HINT, PROFILE_LABEL, codecOf, describe } from '../../quality';
  import type { QualityProfile } from '../../types/QualityProfile';

  const app = getApp();
  const { session } = app;
  const settings = $derived(session.settings);
  const quality = $derived<QualityProfile>(settings?.quality ?? 'standard');
  const inCall = $derived(session.connection !== 'disconnected');
</script>

<div class="label">
  <label for="quality">Sprachqualität</label>
  <Info term="sprachqualitaet" />
</div>
<select
  id="quality"
  value={quality}
  onchange={(e) => session.applySettings({ quality: e.currentTarget.value as QualityProfile })}
>
  {#each PROFILES as p (p.id)}
    <option value={p.id}>{PROFILE_LABEL[p.id]} – {describe(p.codec)}</option>
  {/each}
  {#if quality === 'custom'}
    <option value="custom"
      >{PROFILE_LABEL.custom} – {settings ? describe(settings.voice) : ''}</option
    >
  {/if}
</select>
<p class="note">{PROFILE_HINT[quality]}</p>
{#if settings && codecOf(settings).stereo}
  <p class="note">
    Stereo heißt hier: dasselbe Mikrofonsignal auf zwei Kanälen <Info term="stereo" />. Es misst,
    was zwei Kanäle kosten – ein Stereo-Mikrofon wird nicht gebaut.
  </p>
{/if}
{#if inCall}
  <p class="note">
    Im Call wird die Stimme kurz neu gesendet; die anderen hören eine kleine Pause.
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
