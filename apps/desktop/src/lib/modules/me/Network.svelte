<script lang="ts">
  // Profi → „Netzwerk“: the voice bitrate. The baseline asks for 64–96 kbit/s with RED; the
  // live numbers are in „Diagnose“.
  import { getApp } from '../../app.svelte';
  import Info from '../../Info.svelte';

  const app = getApp();
  const { session } = app;
  const bitrate = $derived(session.settings?.voiceBitrateKbps ?? 80);
</script>

<section class="network" aria-labelledby="network-title">
  <h1 id="network-title">Netzwerk</h1>
  <div class="label">
    <label for="bitrate">Bitrate der Stimme: {bitrate} kbit/s</label>
    <Info term="senderate" />
  </div>
  <input
    id="bitrate"
    type="range"
    min="32"
    max="128"
    step="8"
    value={bitrate}
    onchange={(e) => session.applySettings({ voiceBitrateKbps: Number(e.currentTarget.value) })}
  />
  <p class="note">
    Empfohlen 64–96 kbit/s. Mehr klingt kaum besser, weniger hilft bei schwachem Upload. Gilt ab dem
    nächsten Beitritt zu einem Channel.
  </p>
  <p class="note">
    Jedes Paket trägt zusätzlich eine Kopie des vorigen (RED <Info term="red" />), damit einzelne
    Verluste nicht zu hören sind.
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

  .label {
    display: flex;
    align-items: center;
  }

  label {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  input {
    accent-color: var(--color-accent);
  }

  .note {
    margin: 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .quiet {
    align-self: flex-start;
    margin-top: var(--space-2);
    background: transparent;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }
</style>
