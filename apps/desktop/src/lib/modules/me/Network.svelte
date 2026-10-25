<script lang="ts">
  // Profi → „Netzwerk“: the voice quality profile and the codec values behind it. Changing a
  // value makes the profile „Eigen“; the live numbers are in „Diagnose“.
  import { getApp } from '../../app.svelte';
  import Info from '../../Info.svelte';
  import { OPUS_FIXED, codecOf, matching } from '../../quality';
  import type { VoiceCodec } from '../../types/VoiceCodec';
  import QualityPicker from './QualityPicker.svelte';

  const app = getApp();
  const { session } = app;
  const settings = $derived(session.settings);
  const codec = $derived<VoiceCodec>(
    settings ? codecOf(settings) : { bitrateKbps: 80, stereo: false, red: true, dtx: false },
  );
  /** RED copies are dropped by libwebrtc once primary + copy pass 1 kB: ≈ 200 kbit/s at 20 ms. */
  const redUseless = $derived(codec.red && codec.bitrateKbps > 200);

  function change(patch: Partial<VoiceCodec>) {
    const voice = { ...codec, ...patch };
    return session.applySettings({ voice, quality: matching(voice) });
  }
</script>

<section class="network" aria-labelledby="network-title">
  <h1 id="network-title">Netzwerk</h1>
  <QualityPicker />

  <h2>Die Werte dahinter</h2>
  <div class="label">
    <label for="bitrate">Bitrate: {codec.bitrateKbps} kbit/s</label>
    <Info term="senderate" />
  </div>
  <input
    id="bitrate"
    type="range"
    min="16"
    max="510"
    step="2"
    value={codec.bitrateKbps}
    onchange={(e) => change({ bitrateKbps: Number(e.currentTarget.value) })}
  />
  <p class="note">
    Opus nimmt 16 bis 510 kbit/s, libwebrtc deckelt den Encoder aber bei etwa 256 kbit/s –
    „Ausgehandelte Bitrate“ in der Diagnose zeigt, was er wirklich anpeilt. Für Sprache ist ab etwa
    128 kbit/s kaum noch ein Unterschied zu hören.
  </p>

  <label class="check">
    <input
      type="checkbox"
      checked={codec.stereo}
      onchange={(e) => change({ stereo: e.currentTarget.checked })}
    />
    <span>Stereo (zwei Kanäle, dasselbe Signal) <Info term="stereo" /></span>
  </label>
  <label class="check">
    <input
      type="checkbox"
      checked={codec.red}
      onchange={(e) => change({ red: e.currentTarget.checked })}
    />
    <span>RED – jedes Paket trägt das vorige noch einmal <Info term="red" /></span>
  </label>
  {#if redUseless}
    <p class="note warn">
      Über etwa 200 kbit/s passt die Kopie nicht mehr ins Paket; libwebrtc lässt sie dann
      stillschweigend weg (gemessen: die Senderate bleibt bei der reinen Bitrate).
    </p>
  {/if}
  <label class="check">
    <input
      type="checkbox"
      checked={codec.dtx}
      onchange={(e) => change({ dtx: e.currentTarget.checked })}
    />
    <span>DTX – in Sprechpausen nichts senden <Info term="dtx" /></span>
  </label>

  <h2>Fest in Opus/libwebrtc</h2>
  <dl>
    <dt>Rahmenlänge <Info term="rahmenlaenge" /></dt>
    <dd>{OPUS_FIXED.frameMs} ms</dd>
    <dt>In-band-FEC <Info term="fec" /></dt>
    <dd>{OPUS_FIXED.inbandFec ? 'an' : 'aus'}</dd>
    <dt>Komplexität <Info term="komplexitaet" /></dt>
    <dd>{OPUS_FIXED.complexity} von 10</dd>
    <dt>Abtastrate <Info term="abtastrate" /></dt>
    <dd>48 kHz</dd>
  </dl>
  <p class="note">
    Diese Werte lässt das LiveKit-SDK nicht durch; sie sind libwebrtcs Voreinstellung. Mehr als 48
    kHz brächte bei Opus nichts – der Codec arbeitet intern mit 48 kHz, und oberhalb von 20 kHz hört
    niemand etwas.
  </p>

  <button class="quiet" onclick={() => app.openMe('diagnose')}>Live-Werte in „Diagnose“</button>
</section>

<style>
  .network {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    max-width: 480px;
  }

  h1 {
    margin: 0 0 var(--space-2);
    font-size: var(--font-size-lg);
  }

  h2 {
    margin: var(--space-4) 0 0;
    font-size: var(--font-size-md);
  }

  .label {
    display: flex;
    align-items: center;
  }

  label {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .check {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--color-text);
  }

  .check span {
    display: inline-flex;
    align-items: center;
  }

  input[type='range'] {
    accent-color: var(--color-accent);
  }

  input[type='checkbox'] {
    accent-color: var(--color-accent);
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-1) var(--space-3);
    margin: 0;
    font-size: var(--font-size-sm);
  }

  dt {
    display: flex;
    align-items: center;
    color: var(--color-text-muted);
  }

  dd {
    margin: 0;
  }

  .note {
    margin: 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .warn {
    color: var(--color-warning);
  }

  .quiet {
    align-self: flex-start;
    margin-top: var(--space-2);
    background: transparent;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }
</style>
