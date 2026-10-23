<script lang="ts">
  // Profi → „Signalkette“: the send chain as a row of stages, each with its live level. A
  // click opens the stage's parameters below. What the chain costs (CPU per 10 ms block) and
  // adds (latency) is always on top — the latency budget is 70–120 ms mouth to ear.
  import { getApp } from '../../app.svelte';
  import Info from '../../Info.svelte';
  import Meter from '../../Meter.svelte';
  import type { CompressorSettings } from '../../types/CompressorSettings';
  import type { DenoiserKind } from '../../types/DenoiserKind';
  import type { EqBand } from '../../types/EqBand';

  const app = getApp();
  const { session } = app;

  const STAGES = [
    { id: 'input', label: 'Eingang' },
    { id: 'highpass', label: 'Hochpass' },
    { id: 'denoiser', label: 'Rauschunterdrückung' },
    { id: 'gate', label: 'Gate' },
    { id: 'eq', label: 'EQ' },
    { id: 'compressor', label: 'Kompressor' },
  ] as const;
  type StageId = (typeof STAGES)[number]['id'];

  let open = $state<StageId>('denoiser');

  const settings = $derived(session.settings);
  const level = (id: string) => session.stages.find((s) => s.id === id);
  const fixed = (n: number, digits = 1) => n.toFixed(digits).replace('.', ',');
  /** 3:1, 2,5:1 */
  const ratio = (r: number) => `${Number.isInteger(r) ? r : fixed(r)}:1`;
  const busy = $derived(session.stats?.pumpBusyMeanMs ?? null);
  const busyMax = $derived(session.stats?.pumpBusyMaxMs ?? null);
  const latency = $derived(session.stats?.chainLatencyMs ?? null);

  const denoisers: { id: DenoiserKind; label: string; short: string }[] = [
    { id: 'deepFilter', label: 'DeepFilterNet (stark)', short: 'DeepFilterNet' },
    { id: 'rnnoise', label: 'RNNoise (leicht, wenig CPU)', short: 'RNNoise' },
    { id: 'off', label: 'Aus', short: 'aus' },
  ];

  /** A few words for the state under each stage. */
  function stateOf(id: StageId): string {
    if (!settings) return '';
    switch (id) {
      case 'highpass':
        return settings.highPassHz > 0 ? `${Math.round(settings.highPassHz)} Hz` : 'aus';
      case 'denoiser':
        return denoisers.find((d) => d.id === settings.denoiser)?.short ?? '';
      case 'gate':
        return level('gate')?.active ? 'offen' : 'zu';
      case 'eq':
        return settings.eq.length === 0
          ? 'flach'
          : `${settings.eq.length} ${settings.eq.length === 1 ? 'Band' : 'Bänder'}`;
      case 'compressor':
        return settings.compressor.enabled ? ratio(settings.compressor.ratio) : 'aus';
      default:
        return `Eingang ${settings.inputChannel + 1}`;
    }
  }

  const num = (e: Event) => Number((e.currentTarget as HTMLInputElement).value);

  function setCompressor(patch: Partial<CompressorSettings>) {
    if (settings) void session.applySettings({ compressor: { ...settings.compressor, ...patch } });
  }

  function setBand(index: number, patch: Partial<EqBand>) {
    if (!settings) return;
    const eq = settings.eq.map((b, i) => (i === index ? { ...b, ...patch } : b));
    void session.applySettings({ eq });
  }

  function addBand() {
    if (!settings || settings.eq.length >= 5) return;
    void session.applySettings({ eq: [...settings.eq, { freqHz: 3000, gainDb: 0, q: 1 }] });
  }

  function removeBand(index: number) {
    if (settings) void session.applySettings({ eq: settings.eq.filter((_, i) => i !== index) });
  }
</script>

<section class="chain" aria-labelledby="chain-title">
  <h1 id="chain-title">Signalkette</h1>

  <dl class="cost">
    <div class:warn={busyMax !== null && busyMax > 5}>
      <dt>Rechenzeit je 10-ms-Block</dt>
      <dd>
        {busy === null ? '–' : `${fixed(busy, 2)} ms`}
        <span class="muted">
          (Spitze {busyMax === null ? '–' : `${fixed(busyMax, 2)} ms`}, Budget 5 ms)
        </span>
      </dd>
    </div>
    <div>
      <dt>Zusätzliche Latenz <Info term="latenz" /></dt>
      <dd>{latency === null ? '–' : `${fixed(latency, 0)} ms`}</dd>
    </div>
  </dl>

  <ol class="stages" aria-label="Stufen der Signalkette">
    {#each STAGES as stage (stage.id)}
      {@const l = level(stage.id)}
      <li>
        <button
          class="stage"
          aria-expanded={open === stage.id}
          aria-controls="stage-params"
          onclick={() => (open = stage.id)}
        >
          <span class="name">{stage.label}</span>
          <Meter
            db={l?.levelDb ?? -120}
            active={l?.active ?? false}
            label="Pegel nach {stage.label}"
          />
          <span class="state">{stateOf(stage.id)}</span>
        </button>
      </li>
    {/each}
  </ol>

  <div id="stage-params" class="params">
    {#if !settings}
      <p class="muted">Die Engine startet noch …</p>
    {:else if open === 'input'}
      <h2>Eingang</h2>
      <p class="muted">
        Gerät, Eingang am Gerät und Sprechmodus stehen unter „Audio und Sprechen“.
      </p>
      <button class="quiet" onclick={() => app.openMe('audio')}>Zu „Audio und Sprechen“</button>
    {:else if open === 'highpass'}
      <h2>Hochpass</h2>
      <label class="check">
        <input
          type="checkbox"
          checked={settings.highPassHz > 0}
          onchange={(e) => session.applySettings({ highPassHz: e.currentTarget.checked ? 80 : 0 })}
        />
        An – nimmt Trittschall, Rumpeln und Tischklopfen unter der Stimme weg
      </label>
      {#if settings.highPassHz > 0}
        <label for="hp">Grenzfrequenz: {Math.round(settings.highPassHz)} Hz</label>
        <input
          id="hp"
          type="range"
          min="40"
          max="200"
          step="5"
          value={settings.highPassHz}
          onchange={(e) => session.applySettings({ highPassHz: num(e) })}
        />
      {/if}
    {:else if open === 'denoiser'}
      <h2>Rauschunterdrückung <Info term="rauschunterdrueckung" /></h2>
      <label for="dn">Verfahren</label>
      <select
        id="dn"
        value={settings.denoiser}
        onchange={(e) => session.applySettings({ denoiser: e.currentTarget.value as DenoiserKind })}
      >
        {#each denoisers as d (d.id)}
          <option value={d.id}>{d.label}</option>
        {/each}
      </select>
      <p class="muted">DeepFilterNet kostet etwa 0,7 ms je Block, RNNoise unter 0,1 ms.</p>
    {:else if open === 'gate'}
      <h2>Gate</h2>
      {#if settings.transmitMode === 'voiceActivation'}
        <label for="th">Schwelle: {Math.round(settings.vadThresholdDb)} dB</label>
        <input
          id="th"
          type="range"
          min="-80"
          max="-20"
          step="1"
          value={settings.vadThresholdDb}
          onchange={(e) => session.applySettings({ vadThresholdDb: num(e) })}
        />
        <label for="hold">Nachlauf: {settings.vadHoldMs} ms</label>
        <input
          id="hold"
          type="range"
          min="50"
          max="1000"
          step="50"
          value={settings.vadHoldMs}
          onchange={(e) => session.applySettings({ vadHoldMs: num(e) })}
        />
        <p class="muted">Der Nachlauf hält das Gate über kurze Sprechpausen offen.</p>
      {:else}
        <p class="muted">
          {settings.transmitMode === 'pushToTalk'
            ? 'Taste halten: Das Gate ist offen, solange die Taste gedrückt ist.'
            : 'Immer an: Das Gate ist dauerhaft offen.'}
        </p>
      {/if}
    {:else if open === 'eq'}
      <h2>EQ</h2>
      {#if settings.eq.length === 0}
        <p class="muted">Flach – kein Band gesetzt.</p>
      {/if}
      {#each settings.eq as band, i (i)}
        <fieldset class="band">
          <legend>Band {i + 1}</legend>
          <label>
            Frequenz (Hz)
            <input
              type="number"
              min="20"
              max="20000"
              step="10"
              value={band.freqHz}
              onchange={(e) => setBand(i, { freqHz: num(e) })}
            />
          </label>
          <label>
            Verstärkung (dB)
            <input
              type="number"
              min="-12"
              max="12"
              step="0.5"
              value={band.gainDb}
              onchange={(e) => setBand(i, { gainDb: num(e) })}
            />
          </label>
          <label>
            Güte (Q)
            <input
              type="number"
              min="0.1"
              max="10"
              step="0.1"
              value={band.q}
              onchange={(e) => setBand(i, { q: num(e) })}
            />
          </label>
          <button class="quiet" onclick={() => removeBand(i)}>Band {i + 1} entfernen</button>
        </fieldset>
      {/each}
      <button class="quiet" onclick={addBand} disabled={settings.eq.length >= 5}>
        Band hinzufügen
      </button>
    {:else if open === 'compressor'}
      {@const c = settings.compressor}
      <h2>Kompressor</h2>
      <label class="check">
        <input
          type="checkbox"
          checked={c.enabled}
          onchange={(e) => setCompressor({ enabled: e.currentTarget.checked })}
        />
        An – gleicht laut und leise an
      </label>
      {#if c.enabled}
        <label for="ct">Schwelle: {Math.round(c.thresholdDb)} dB</label>
        <input
          id="ct"
          type="range"
          min="-40"
          max="0"
          step="1"
          value={c.thresholdDb}
          onchange={(e) => setCompressor({ thresholdDb: num(e) })}
        />
        <label for="cr">Verhältnis: {ratio(c.ratio)}</label>
        <input
          id="cr"
          type="range"
          min="1"
          max="10"
          step="0.5"
          value={c.ratio}
          onchange={(e) => setCompressor({ ratio: num(e) })}
        />
        <label for="ca">Attack: {Math.round(c.attackMs)} ms</label>
        <input
          id="ca"
          type="range"
          min="1"
          max="50"
          step="1"
          value={c.attackMs}
          onchange={(e) => setCompressor({ attackMs: num(e) })}
        />
        <label for="cre">Release: {Math.round(c.releaseMs)} ms</label>
        <input
          id="cre"
          type="range"
          min="20"
          max="500"
          step="10"
          value={c.releaseMs}
          onchange={(e) => setCompressor({ releaseMs: num(e) })}
        />
        <label for="cm">Aufholverstärkung: {fixed(c.makeupDb)} dB</label>
        <input
          id="cm"
          type="range"
          min="0"
          max="12"
          step="0.5"
          value={c.makeupDb}
          onchange={(e) => setCompressor({ makeupDb: num(e) })}
        />
      {/if}
    {/if}
  </div>

  <p class="muted later">Effekte und Plugins (VST/CLAP) kommen als eigene Stufen in Phase 3.</p>
</section>

<style>
  .chain {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    max-width: 720px;
  }

  h1 {
    margin: 0;
    font-size: var(--font-size-lg);
  }

  h2 {
    display: flex;
    align-items: center;
    margin: 0;
    font-size: var(--font-size-md);
  }

  .cost {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-4);
    margin: 0;
  }

  .cost dt {
    display: flex;
    align-items: center;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .cost dd {
    margin: 0;
    font-family: var(--font-mono);
  }

  .cost .warn dd {
    color: var(--color-warning);
  }

  .stages {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(112px, 1fr));
    gap: var(--space-2);
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .stage {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    width: 100%;
    height: 100%;
    text-align: left;
    background: var(--color-surface);
    border: 1px solid var(--color-border);
  }

  .stage[aria-expanded='true'] {
    border-color: var(--color-accent);
  }

  .name {
    font-size: var(--font-size-sm);
    font-weight: 600;
  }

  .state,
  .muted {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .muted {
    margin: 0;
  }

  .params {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-3) var(--space-4);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
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

  select,
  input[type='number'] {
    font: inherit;
    color: inherit;
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: var(--space-1) var(--space-2);
  }

  input {
    accent-color: var(--color-accent);
  }

  .band {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: var(--space-3);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: var(--space-2) var(--space-3);
  }

  .band label {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .band input {
    width: 7em;
  }

  .quiet {
    align-self: flex-start;
    background: transparent;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .later {
    margin-top: var(--space-2);
  }
</style>
