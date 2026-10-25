<script lang="ts">
  // Everything a tester can read out when something sounds wrong: connection, network,
  // devices, and what the engine logged. The same numbers go into the log file once per
  // second, so "it crackled around nine" can be looked up afterwards.
  import type { AppInfo } from './engine';
  import Info from './Info.svelte';
  import type { Session } from './session.svelte';
  import { PROFILE_LABEL, codecOf, describe } from './quality';
  import type { DeviceStats } from './types/DeviceStats';

  let { session, info }: { session: Session; info: AppInfo | null } = $props();

  const connectionLabel = {
    disconnected: 'getrennt',
    connecting: 'verbinde',
    connected: 'verbunden',
    reconnecting: 'verbinde neu',
  } as const;

  /** `term` = glossary slug (docs/hilfe/glossar.md) shown as ⓘ next to the label. */
  type Row = { label: string; value: string; warn?: boolean; term?: string };

  const fixed = (n: number, digits = 1) => n.toFixed(digits).replace('.', ',');
  /** +6,0 / −2,5 / 0,0 */
  const signed = (n: number) => (n > 0.05 ? '+' : n < -0.05 ? '−' : '') + fixed(Math.abs(n));
  const net = $derived(session.stats?.network ?? null);
  /** „Standard (80 kbit/s · Mono · RED)“ — what the settings say we send. */
  const profile = $derived(
    session.settings
      ? `${PROFILE_LABEL[session.settings.quality]} (${describe(codecOf(session.settings))})`
      : '–',
  );

  const network = $derived<Row[]>(
    net
      ? [
          {
            label: 'Laufzeit zum Server',
            value: `${fixed(net.rttMs, 0)} ms`,
            warn: net.rttMs > 150,
            term: 'laufzeit-zum-server',
          },
          {
            label: 'Paketverlust Empfang',
            value: `${fixed(net.lossPercentRecent)} %`,
            warn: net.lossPercentRecent > 5,
            term: 'paketverlust',
          },
          {
            label: 'Paketverlust Senden',
            value: `${fixed(net.uplinkLossPercent)} %`,
            warn: net.uplinkLossPercent > 5,
            term: 'paketverlust',
          },
          {
            label: 'Ersetzte Samples',
            value: `${fixed(net.concealedPercentRecent)} % (gesamt ${fixed(net.concealedPercentTotal)} %)`,
            warn: net.concealedPercentRecent > 2,
            term: 'ersetzte-samples',
          },
          {
            label: 'Jitter',
            value: `${fixed(net.jitterMs, 0)} ms`,
            warn: net.jitterMs > 30,
            term: 'jitter',
          },
          {
            label: 'Jitterbuffer',
            value: `${fixed(net.jitterBufferMs, 0)} ms`,
            warn: net.jitterBufferMs > 120,
            term: 'jitterbuffer',
          },
          { label: 'Sprachqualität', value: profile, term: 'sprachqualitaet' },
          {
            // The encoder's own target against the profile: is the bitrate really in use?
            label: 'Ausgehandelte Bitrate',
            value:
              net.sendTargetKbps > 0
                ? `${fixed(net.sendTargetKbps, 0)} kbit/s · ${net.sendChannels === 2 ? 'Stereo' : 'Mono'}`
                : '–',
            warn:
              net.sendTargetKbps > 0 &&
              session.settings !== null &&
              net.sendTargetKbps < codecOf(session.settings).bitrateKbps * 0.8,
            term: 'ausgehandelte-bitrate',
          },
          {
            label: 'Senderate',
            value: `${fixed(net.sendKbpsRecent, 0)} kbit/s (Nutzlast ${fixed(net.sendPayloadKbpsRecent, 0)})`,
            term: 'senderate',
          },
          { label: 'Codec', value: net.sendCodec || '–', term: 'codec' },
          {
            label: 'Pakete',
            value: `${net.packetsSent} gesendet · ${net.packetsReceived} empfangen · ${net.packetsLost} verloren`,
          },
        ]
      : [],
  );

  function deviceRows(device: DeviceStats | null, xrunLabel: string): Row[] {
    if (!device) return [{ label: 'Gerät', value: 'nicht geöffnet', warn: true }];
    const ms = fixed((device.callbackFrames / device.sampleRate) * 1000);
    return [
      { label: 'Gerät', value: device.name },
      {
        label: 'Format',
        value: `${device.sampleRate} Hz · ${device.channels} Kanäle`,
        term: 'abtastrate',
      },
      { label: 'Puffer', value: `${device.callbackFrames} Frames (${ms} ms)`, term: 'puffer' },
      { label: xrunLabel, value: String(device.xruns), warn: device.xruns > 0, term: 'aussetzer' },
    ];
  }

  const stats = $derived(session.stats);
  const pipeline = $derived<Row[]>(
    stats
      ? [
          {
            label: 'Längster Sende-Abstand',
            value: `${fixed(stats.sendIntervalMaxMs)} ms`,
            warn: stats.sendIntervalMaxMs > 30,
          },
          {
            // The whole send chain must stay under 5 ms per 10 ms block (planning doc).
            label: 'Sendekette pro Block',
            value: `Ø ${fixed(stats.pumpBusyMeanMs * 1000, 0)} µs · max ${fixed(stats.pumpBusyMaxMs * 1000, 0)} µs`,
            warn: stats.pumpBusyMaxMs > 5,
          },
          {
            label: 'Verworfene Sende-Blöcke',
            value: String(stats.sendBlocksDropped),
            warn: stats.sendBlocksDropped > 0,
          },
          {
            label: 'Verworfene Empfangs-Frames',
            value: String(stats.playoutFramesDropped),
            warn: stats.playoutFramesDropped > 0,
          },
          {
            label: 'Sprünge in der Wiedergabe',
            value: String(stats.playoutSkips),
            warn: stats.playoutSkips > 0,
          },
          {
            // A4: who arrives too quiet or too loud — the leveler's gain per speaker.
            label: 'Lautstärke-Angleich',
            value:
              session.settings?.autoLevel === false
                ? 'aus'
                : stats.speakers.length === 0
                  ? '–'
                  : stats.speakers
                      .map((s) => `${s.identity} ${signed(s.levelerDb)} dB`)
                      .join(' · '),
            term: 'lautstaerke-angleich',
          },
          {
            // A5: the local path's latency while the Signalkette page listens to itself.
            label: 'Mich selbst hören',
            value: stats.monitor
              ? `an · etwa ${fixed(stats.monitor.latencyMs, 0)} ms Mikro bis Kopfhörer · ${stats.monitor.underruns} Aussetzer · ${stats.monitor.droppedBlocks} verworfen`
              : 'aus',
            warn: (stats.monitor?.underruns ?? 0) + (stats.monitor?.droppedBlocks ?? 0) > 0,
            term: 'mich-selbst-hoeren',
          },
          {
            // A6: how far each speaker's clock is from ours (ppm the playback is sped up
            // or slowed down) and how much of their voice is queued. Orange when a ratio
            // sits at its limit: the compensation cannot keep up, skips will follow.
            label: 'Drift-Ausgleich',
            value:
              stats.speakers.length === 0
                ? '–'
                : stats.speakers
                    .map((s) => {
                      const ppm = (s.driftRatio - 1) * 1e6;
                      const sign = ppm > 0.5 ? '+' : ppm < -0.5 ? '−' : '';
                      return `${s.identity} ${sign}${fixed(Math.abs(ppm), 0)} ppm · ${fixed(s.playoutFillMs, 0)} ms`;
                    })
                    .join(' · '),
            // 0.0049, not 0.005: the engine reports the f32-rounded clamp (1.00499999…).
            warn: stats.speakers.some((s) => Math.abs(s.driftRatio - 1) >= 0.0049),
            term: 'drift-ausgleich',
          },
        ]
      : [],
  );

  const timeline = $derived(session.log.slice(-120).reverse());
  const clock = (ms: number) => new Date(ms).toLocaleTimeString('de-DE');

  let exported = $state<string | null>(null);
  let exportProblem = $state<string | null>(null);

  async function exportLogs() {
    exportProblem = null;
    try {
      exported = await session.exportLogs();
    } catch (reason) {
      exportProblem = String(reason);
    }
  }
</script>

{#snippet rows(list: Row[])}
  <dl>
    {#each list as row (row.label)}
      <dt>
        {row.label}{#if row.term}<Info term={row.term} />{/if}
      </dt>
      <dd class:warn={row.warn}>{row.value}</dd>
    {/each}
  </dl>
{/snippet}

<section class="debug" aria-labelledby="debug-title">
  <h1 id="debug-title">Diagnose</h1>
  <section>
    <h2>Verbindung</h2>
    {@render rows([
      { label: 'Status', value: connectionLabel[session.connection] },
      { label: 'Raum', value: session.room ?? '–' },
      { label: 'Ich', value: session.identity ?? '–' },
    ])}
  </section>

  <section>
    <h2>Netz</h2>
    {#if network.length}
      {@render rows(network)}
    {:else}
      <p class="none">Erst im Raum verfügbar.</p>
    {/if}
  </section>

  <section>
    <h2>Mikrofon</h2>
    {@render rows(deviceRows(stats?.capture ?? null, 'Verlorene Samples'))}
  </section>

  <section>
    <h2>Ausgabe</h2>
    {@render rows(deviceRows(stats?.playout ?? null, 'Aussetzer'))}
  </section>

  <section>
    <h2>Engine</h2>
    {@render rows(pipeline)}
  </section>

  <section>
    <h2>Diagnose</h2>
    <button onclick={exportLogs}>Diagnose-Datei speichern</button>
    {#if exported}
      <p class="path" role="status">Gespeichert: {exported}</p>
    {/if}
    {#if exportProblem}
      <p class="warn" role="alert">Speichern fehlgeschlagen: {exportProblem}</p>
    {/if}
    {#if info}
      <p class="none">
        Version {info.version} · Protokoll {info.protocolVersion}
        {#if info.logDir}<br />Logs: <span class="path">{info.logDir}</span>{/if}
      </p>
    {/if}
  </section>

  <section>
    <h2>Verlauf</h2>
    {#if timeline.length === 0}
      <p class="none">Noch keine Einträge.</p>
    {:else}
      <ol class="timeline">
        {#each timeline as entry, index (index)}
          <li class={entry.level}>
            <time>{clock(entry.timeMs)}</time>
            <span>{entry.message}</span>
          </li>
        {/each}
      </ol>
    {/if}
  </section>
</section>

<style>
  .debug > h1 {
    grid-column: 1 / -1;
    margin: 0 0 var(--space-2);
    font-size: var(--font-size-lg);
  }

  .debug {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: var(--space-2) var(--space-6);
    font-size: var(--font-size-sm);
    user-select: text;
    scrollbar-color: var(--color-border) transparent;
  }

  /* In the grid every block stands on its own; the history spans the full width. */
  .debug > section:last-of-type {
    grid-column: 1 / -1;
  }

  h2 {
    margin: 0 0 var(--space-2);
    font-size: var(--font-size-sm);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--color-text-muted);
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-1) var(--space-3);
    margin: 0;
  }

  dt {
    color: var(--color-text-muted);
  }

  dd {
    margin: 0;
    text-align: right;
    font-family: var(--font-mono);
    overflow-wrap: anywhere;
  }

  .warn {
    color: var(--color-warning);
  }

  .none {
    margin: var(--space-2) 0 0;
    color: var(--color-text-muted);
  }

  .path {
    font-family: var(--font-mono);
    overflow-wrap: anywhere;
  }

  .timeline {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .timeline li {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-2);
    overflow-wrap: anywhere;
  }

  time {
    color: var(--color-text-muted);
    font-family: var(--font-mono);
  }

  .timeline .warn span {
    color: var(--color-warning);
  }

  .timeline .error span {
    color: var(--color-danger);
  }
</style>
