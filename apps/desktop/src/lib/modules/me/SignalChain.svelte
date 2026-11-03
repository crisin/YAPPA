<script lang="ts">
  // Profi → „Signalkette“: the send chain as a strip of stages, each with its live level, and
  // below it the chosen stage as a rack unit — knobs, program buttons, a power switch, the
  // EQ as a curve you can drag. What the chain costs (CPU per 10 ms block) and adds (latency)
  // is always on top — the latency budget is 70–120 ms mouth to ear.
  import { getApp } from '../../app.svelte';
  import Info from '../../Info.svelte';
  import Meter from '../../Meter.svelte';
  import EqCurve from '../../rack/EqCurve.svelte';
  import Knob from '../../rack/Knob.svelte';
  import ProgramButtons from '../../rack/ProgramButtons.svelte';
  import type { CompressorSettings } from '../../types/CompressorSettings';
  import type { DenoiserKind } from '../../types/DenoiserKind';
  import type { EffectSettings } from '../../types/EffectSettings';
  import type { EqBand } from '../../types/EqBand';
  import type { Scale } from '../../types/Scale';
  import type { VoiceEffect } from '../../types/VoiceEffect';

  const app = getApp();
  const { session } = app;

  const STAGES = [
    { id: 'input', label: 'Eingang' },
    { id: 'highpass', label: 'Hochpass' },
    { id: 'denoiser', label: 'Rauschunterdrückung' },
    { id: 'gate', label: 'Gate' },
    { id: 'eq', label: 'EQ' },
    { id: 'compressor', label: 'Kompressor' },
    { id: 'effect', label: 'Effekt' },
    { id: 'limiter', label: 'Limiter' },
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

  // „Mich selbst hören“ (A5): the chain's output goes into the own headphones while this
  // page is open and the switch is on — off the moment the page closes. With speakers
  // (onboarding choice) the microphone would hear the output again: the switch starts off
  // and a warning stands next to it.
  const speakers = $derived(app.appSettings?.listening === 'speakers');
  const monitorWanted = $derived(app.hearMyself ?? !speakers);
  const monitorLatency = $derived(session.stats?.monitor?.latencyMs ?? null);
  $effect(() => {
    void session.setMonitor(monitorWanted);
    return () => void session.setMonitor(false);
  });

  const denoisers: { id: DenoiserKind; label: string; short: string }[] = [
    { id: 'deepFilter', label: 'DeepFilterNet', short: 'DeepFilterNet' },
    { id: 'rnnoise', label: 'RNNoise', short: 'RNNoise' },
    { id: 'off', label: 'Aus', short: 'aus' },
  ];

  // The effect slot (ADR-012): one effect between compressor and limiter, for the evening,
  // not for the studio. The pitch shifter delays the voice by half its window (15 ms).
  const effects: { id: VoiceEffect; label: string; short: string }[] = [
    { id: 'off', label: 'Aus', short: 'aus' },
    { id: 'pitch', label: 'Tonhöhe', short: 'Tonhöhe' },
    { id: 'robot', label: 'Roboter', short: 'Roboter' },
    { id: 'hall', label: 'Halle', short: 'Halle' },
    { id: 'radio', label: 'Funkgerät', short: 'Funk' },
    { id: 'autotune', label: 'Autotune', short: 'Autotune' },
    { id: 'harmony', label: 'Zweite Stimme', short: '2. Stimme' },
  ];
  // Autotune and „Zweite Stimme“ share key and scale. German note names: B is „H“, B♭ is „B“.
  const KEYS = ['C', 'C♯', 'D', 'D♯', 'E', 'F', 'F♯', 'G', 'G♯', 'A', 'B', 'H'] as const;
  const keys = KEYS.map((label, i) => ({ id: String(i), label }));
  // `notes`: how many steps make an octave; in a short scale the knob's end is the octave.
  const scales: { id: Scale; label: string; short: string; notes: number }[] = [
    { id: 'chromatic', label: 'Chromatisch', short: 'chromatisch', notes: 12 },
    { id: 'major', label: 'Dur', short: 'Dur', notes: 7 },
    { id: 'naturalMinor', label: 'Moll', short: 'Moll', notes: 7 },
    { id: 'harmonicMinor', label: 'Harmonisch Moll', short: 'harm. Moll', notes: 7 },
    { id: 'pentatonicMajor', label: 'Pentatonik Dur', short: 'Pent. Dur', notes: 5 },
    { id: 'pentatonicMinor', label: 'Pentatonik Moll', short: 'Pent. Moll', notes: 5 },
    { id: 'blues', label: 'Blues', short: 'Blues', notes: 6 },
  ];
  const scaleOf = (id: Scale) => scales.find((s) => s.id === id) ?? scales[1];
  /** „C-Dur“, „A-Moll“, „chromatisch“. */
  const keyScale = (key: number, scale: Scale) =>
    scale === 'chromatic' ? 'chromatisch' : `${KEYS[key] ?? 'C'}-${scaleOf(scale).short}`;
  // Interval names hold for seven-note scales; in a pentatonic or blues scale two steps
  // are not a third, so there only the steps are shown.
  const INTERVALS = ['Prime', 'Sekunde', 'Terz', 'Quarte', 'Quinte', 'Sexte', 'Septime', 'Oktave'];
  const sign = (n: number) => (n > 0 ? '+' : n < 0 ? '−' : '±');
  /** The interval's name: Terz … Oktave in a seven-note scale, else only the octave. */
  function intervalName(steps: number, scale: Scale): string | null {
    const n = Math.abs(Math.round(steps));
    const { notes } = scaleOf(scale);
    if (n >= notes) return 'Oktave';
    return notes === 7 ? INTERVALS[n] : null;
  }
  /** „+Terz“, „−Quinte“, „+2 Stufen“ — the stage text. */
  const intervalShort = (steps: number, scale: Scale) => {
    const n = Math.abs(Math.round(steps));
    return `${sign(steps)}${intervalName(steps, scale) ?? `${n} ${n === 1 ? 'Stufe' : 'Stufen'}`}`;
  };
  /** „+2 · Terz“ for the knob. */
  const intervalLong = (steps: number, scale: Scale) => {
    const n = Math.abs(Math.round(steps));
    const name = intervalName(steps, scale);
    const base = `${sign(steps)}${n} ${n === 1 ? 'Stufe' : 'Stufen'}`;
    return name ? `${sign(steps)}${n} · ${name}` : base;
  };
  const retune = (v: number) => (v === 0 ? 'hart' : percent(v));
  const semitones = (n: number) =>
    `${n > 0 ? '+' : n < 0 ? '−' : ''}${Math.abs(Math.round(n))} ${Math.abs(Math.round(n)) === 1 ? 'Halbton' : 'Halbtöne'}`;
  const signed = (n: number, digits = 0) =>
    `${n > 0 ? '+' : n < 0 ? '−' : ''}${fixed(Math.abs(n), digits)}`;
  const percent = (x: number) => `${Math.round(x * 100)} %`;
  const hz = (f: number) =>
    f >= 1000 ? `${fixed(f / 1000, f >= 10_000 ? 1 : 2)} kHz` : `${Math.round(f)} Hz`;
  const ms = (t: number) => `${Math.round(t)} ms`;
  const db = (v: number, digits = 0) => `${fixed(v, digits)} dB`;

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
      case 'effect':
        switch (settings.effect.kind) {
          case 'pitch':
            return semitones(settings.effect.semitones);
          case 'autotune':
            return `Autotune ${keyScale(settings.effect.key, settings.effect.scale)}`;
          case 'harmony':
            return intervalShort(settings.effect.harmony, settings.effect.scale);
          default:
            return effects.find((e) => e.id === settings.effect.kind)?.short ?? '';
        }
      case 'limiter':
        return '−1 dB';
      default:
        return `Eingang ${settings.inputChannel + 1}`;
    }
  }

  /** The LED on a stage: lit while the stage does something to the signal. */
  function lit(id: StageId): boolean {
    if (!settings) return false;
    switch (id) {
      case 'highpass':
        return settings.highPassHz > 0;
      case 'denoiser':
        return settings.denoiser !== 'off';
      case 'gate':
        return level('gate')?.active ?? false;
      case 'eq':
        return settings.eq.length > 0;
      case 'compressor':
        return settings.compressor.enabled;
      case 'effect':
        return settings.effect.kind !== 'off';
      default:
        return true;
    }
  }

  function setCompressor(patch: Partial<CompressorSettings>) {
    if (settings) void session.applySettings({ compressor: { ...settings.compressor, ...patch } });
  }

  function setEffect(patch: Partial<EffectSettings>) {
    if (settings) void session.applySettings({ effect: { ...settings.effect, ...patch } });
  }

  function setBand(index: number, patch: Partial<EqBand>) {
    if (!settings) return;
    const eq = settings.eq.map((b, i) => (i === index ? { ...b, ...patch } : b));
    void session.applySettings({ eq });
  }

  /** A new band sits where no band is yet — five spots across the voice. */
  const BAND_SPOTS = [3000, 200, 800, 6000, 12_000];
  function addBand() {
    if (!settings || settings.eq.length >= 5) return;
    const taken = settings.eq.map((b) => b.freqHz);
    const freqHz = BAND_SPOTS.find((f) => !taken.includes(f)) ?? 3000;
    void session.applySettings({ eq: [...settings.eq, { freqHz, gainDb: 0, q: 1 }] });
  }

  function removeBand(index: number) {
    if (settings) void session.applySettings({ eq: settings.eq.filter((_, i) => i !== index) });
  }
</script>

<section class="chain" aria-labelledby="chain-title">
  <h1 id="chain-title">Signalkette</h1>

  <div class="monitor">
    <label class="check">
      <input
        type="checkbox"
        checked={session.monitor}
        onchange={(e) => (app.hearMyself = e.currentTarget.checked)}
      />
      Mich selbst hören
    </label>
    <Info term="mich-selbst-hoeren" />
    {#if session.monitor && monitorLatency !== null}
      <span class="muted">etwa {fixed(monitorLatency, 0)} ms Mikro bis Kopfhörer</span>
    {/if}
    {#if speakers}
      <p class="muted warn" role="note">
        Du hast bei der Einrichtung „Lautsprecher“ gewählt. Mit Lautsprechern hört dein Mikro die
        eigene Stimme wieder – das gibt eine Rückkopplung (Pfeifen). Nur mit Kopfhörern einschalten.
      </p>
    {/if}
  </div>

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
          <span class="head">
            <span class="led" class:lit={lit(stage.id)} aria-hidden="true"></span>
            <span class="name">{stage.label}</span>
          </span>
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

  <div id="stage-params" class="unit">
    {#if !settings}
      <p class="muted">Die Engine startet noch …</p>
    {:else if open === 'input'}
      <h2>Eingang</h2>
      <p class="muted">
        Gerät, Eingang am Gerät und Sprechmodus stehen unter „Audio und Sprechen“.
      </p>
      <button class="quiet" onclick={() => app.openMe('audio')}>Zu „Audio und Sprechen“</button>
    {:else if open === 'highpass'}
      <div class="head">
        <h2>Hochpass</h2>
        <label class="power">
          <input
            type="checkbox"
            checked={settings.highPassHz > 0}
            onchange={(e) =>
              session.applySettings({ highPassHz: e.currentTarget.checked ? 80 : 0 })}
          />
          <span class="led" class:lit={settings.highPassHz > 0} aria-hidden="true"></span>
          An
        </label>
      </div>
      <div class="knobs">
        <Knob
          id="hp"
          label="Grenzfrequenz"
          value={settings.highPassHz}
          min={40}
          max={200}
          step={5}
          defaultValue={80}
          format={(v) => `${Math.round(v)} Hz`}
          disabled={settings.highPassHz === 0}
          onchange={(v) => session.applySettings({ highPassHz: v })}
        />
        <p class="muted hint">Nimmt Trittschall, Rumpeln und Tischklopfen unter der Stimme weg.</p>
      </div>
    {:else if open === 'denoiser'}
      <div class="head">
        <h2>Rauschunterdrückung <Info term="rauschunterdrueckung" /></h2>
      </div>
      <ProgramButtons
        label="Verfahren"
        options={denoisers}
        value={settings.denoiser}
        onchange={(id) => session.applySettings({ denoiser: id })}
      />
      <p class="muted">
        DeepFilterNet ist stark und kostet etwa 0,7 ms je Block, RNNoise ist leicht und kostet unter
        0,1 ms.
      </p>
    {:else if open === 'gate'}
      <div class="head">
        <h2>Gate</h2>
      </div>
      {#if settings.transmitMode === 'voiceActivation'}
        <div class="knobs">
          <Knob
            id="th"
            label="Schwelle"
            value={settings.vadThresholdDb}
            min={-80}
            max={-20}
            step={1}
            defaultValue={-50}
            format={(v) => db(v)}
            onchange={(v) => session.applySettings({ vadThresholdDb: v })}
          />
          <Knob
            id="hold"
            label="Nachlauf"
            value={settings.vadHoldMs}
            min={50}
            max={1000}
            step={50}
            defaultValue={300}
            format={ms}
            onchange={(v) => session.applySettings({ vadHoldMs: v })}
          />
          <p class="muted hint">Der Nachlauf hält das Gate über kurze Sprechpausen offen.</p>
        </div>
      {:else}
        <p class="muted">
          {settings.transmitMode === 'pushToTalk'
            ? 'Taste halten: Das Gate ist offen, solange die Taste gedrückt ist.'
            : 'Immer an: Das Gate ist dauerhaft offen.'}
        </p>
      {/if}
    {:else if open === 'eq'}
      <div class="head">
        <h2>EQ</h2>
        <button class="quiet" onclick={addBand} disabled={settings.eq.length >= 5}>
          Band hinzufügen
        </button>
      </div>
      <EqCurve bands={settings.eq} onchange={setBand} />
      {#if settings.eq.length === 0}
        <p class="muted">Flach – kein Band gesetzt. Bis zu fünf Bänder, jedes eine Glocke.</p>
      {:else}
        <p class="muted hint">
          Die Punkte in der Kurve lassen sich ziehen: seitlich die Frequenz, hoch und runter die
          Verstärkung.
        </p>
      {/if}
      {#each settings.eq as band, i (i)}
        <fieldset class="band">
          <legend>Band {i + 1}</legend>
          <div class="knobs">
            <Knob
              id="eq{i}f"
              label="Frequenz"
              value={band.freqHz}
              min={20}
              max={20000}
              step={1}
              log
              defaultValue={3000}
              format={hz}
              onchange={(v) => setBand(i, { freqHz: v })}
            />
            <Knob
              id="eq{i}g"
              label="Verstärkung"
              value={band.gainDb}
              min={-12}
              max={12}
              step={0.5}
              bipolar
              defaultValue={0}
              format={(v) => `${signed(v, 1)} dB`}
              onchange={(v) => setBand(i, { gainDb: v })}
            />
            <Knob
              id="eq{i}q"
              label="Güte (Q)"
              value={band.q}
              min={0.1}
              max={10}
              step={0.1}
              log
              defaultValue={1}
              format={(v) => fixed(v, 1)}
              onchange={(v) => setBand(i, { q: v })}
            />
          </div>
          <button class="quiet" onclick={() => removeBand(i)}>Band {i + 1} entfernen</button>
        </fieldset>
      {/each}
    {:else if open === 'effect'}
      {@const fx = settings.effect}
      <div class="head">
        <h2>Effekt <Info term="stimmeffekt" /></h2>
      </div>
      <ProgramButtons
        label="Effekt"
        options={effects}
        value={fx.kind}
        onchange={(kind) => setEffect({ kind })}
      />
      {#if fx.kind === 'pitch'}
        <div class="knobs">
          <Knob
            id="fxst"
            label="Tonhöhe"
            value={fx.semitones}
            min={-12}
            max={12}
            step={1}
            bipolar
            defaultValue={0}
            size={72}
            format={semitones}
            onchange={(v) => setEffect({ semitones: v })}
          />
          <p class="muted hint">
            Minus macht tiefer, Plus höher. Kostet etwa 15 ms Latenz und flattert bei großen
            Sprüngen – gedacht für den Abend, nicht fürs Singen.
          </p>
        </div>
      {:else if fx.kind === 'autotune' || fx.kind === 'harmony'}
        <ProgramButtons
          label="Tonart"
          options={keys}
          value={String(fx.key)}
          onchange={(id) => setEffect({ key: Number(id) })}
        />
        <ProgramButtons
          label="Skala"
          options={scales}
          value={fx.scale}
          onchange={(scale) => setEffect({ scale })}
        />
        <div class="knobs">
          {#if fx.kind === 'autotune'}
            <Knob
              id="fxrt"
              label="Tempo"
              value={fx.retune}
              min={0}
              max={1}
              step={0.05}
              defaultValue={0.3}
              size={72}
              format={retune}
              onchange={(v) => setEffect({ retune: v })}
            />
            <p class="muted hint">
              Die Stimme rastet auf die Töne von {keyScale(fx.key, fx.scale)} ein. „Tempo“ sagt, wie schnell:
              „hart“ ist der Radio-Effekt, 100 % gleitet in etwa 200 ms hin. Trifft nur, was einen Ton
              hat – Singen und lange Vokale, nicht schnelles Reden. Kostet etwa 15 ms Latenz.
            </p>
          {:else}
            <Knob
              id="fxiv"
              label="Intervall"
              value={fx.harmony}
              min={-7}
              max={7}
              step={1}
              bipolar
              defaultValue={2}
              size={72}
              format={(v) => intervalLong(v, fx.scale)}
              onchange={(v) => setEffect({ harmony: v })}
            />
            <Knob
              id="fxam"
              label="Stärke"
              value={fx.amount}
              min={0}
              max={1}
              step={0.05}
              defaultValue={0.5}
              size={72}
              format={percent}
              onchange={(v) => setEffect({ amount: v })}
            />
            <p class="muted hint">
              Eine zweite Stimme singt in {keyScale(fx.key, fx.scale)} mit – so viele Stufen über (Plus)
              oder unter (Minus) deiner. Sie schweigt bei Konsonanten und in Pausen; deine eigene Stimme
              geht unverzögert raus.
            </p>
          {/if}
        </div>
      {:else if fx.kind !== 'off'}
        <div class="knobs">
          <Knob
            id="fxam"
            label="Stärke"
            value={fx.amount}
            min={0}
            max={1}
            step={0.05}
            defaultValue={0.5}
            size={72}
            format={percent}
            onchange={(v) => setEffect({ amount: v })}
          />
        </div>
      {/if}
      {#if fx.kind !== 'off'}
        <p class="muted">
          Die anderen hören dich so; mit „Mich selbst hören“ hörst du es selbst. Der Limiter
          dahinter fängt, was der Effekt zu laut macht.
        </p>
      {/if}
    {:else if open === 'limiter'}
      <h2>Limiter</h2>
      <p class="muted">
        Immer an, ohne Regler: hält jede Spitze 1 dB unter dem Maximum, damit ein heißes Mikrofon
        oder die Anhebung des Kompressors nie übersteuert. Greift sofort, lässt innerhalb von 80 ms
        wieder los und kostet keine Latenz.
      </p>
    {:else if open === 'compressor'}
      {@const c = settings.compressor}
      <div class="head">
        <h2>Kompressor</h2>
        <label class="power">
          <input
            type="checkbox"
            checked={c.enabled}
            onchange={(e) => setCompressor({ enabled: e.currentTarget.checked })}
          />
          <span class="led" class:lit={c.enabled} aria-hidden="true"></span>
          An
        </label>
      </div>
      <div class="knobs">
        <Knob
          id="ct"
          label="Schwelle"
          value={c.thresholdDb}
          min={-40}
          max={0}
          step={1}
          defaultValue={-18}
          format={(v) => db(v)}
          disabled={!c.enabled}
          onchange={(v) => setCompressor({ thresholdDb: v })}
        />
        <Knob
          id="cr"
          label="Verhältnis"
          value={c.ratio}
          min={1}
          max={10}
          step={0.5}
          defaultValue={3}
          format={ratio}
          disabled={!c.enabled}
          onchange={(v) => setCompressor({ ratio: v })}
        />
        <Knob
          id="ca"
          label="Attack"
          value={c.attackMs}
          min={1}
          max={50}
          step={1}
          defaultValue={5}
          format={ms}
          disabled={!c.enabled}
          onchange={(v) => setCompressor({ attackMs: v })}
        />
        <Knob
          id="cre"
          label="Release"
          value={c.releaseMs}
          min={20}
          max={500}
          step={10}
          defaultValue={120}
          format={ms}
          disabled={!c.enabled}
          onchange={(v) => setCompressor({ releaseMs: v })}
        />
        <Knob
          id="cm"
          label="Aufholen"
          value={c.makeupDb}
          min={0}
          max={12}
          step={0.5}
          defaultValue={3}
          format={(v) => `${signed(v, 1)} dB`}
          disabled={!c.enabled}
          onchange={(v) => setCompressor({ makeupDb: v })}
        />
      </div>
      <p class="muted">
        Gleicht laut und leise an: Was über die Schwelle geht, wird im Verhältnis leiser, „Aufholen“
        hebt alles wieder an.
      </p>
    {/if}
  </div>

  <p class="muted later">
    Regler: ziehen (Shift = fein), Mausrad, Pfeiltasten; Doppelklick setzt zurück, ein Klick auf die
    Zahl tippt sie ein. Eigene Plugins (CLAP) kommen später in denselben Effekt-Slot.
  </p>
</section>

<style>
  .chain {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    max-width: 760px;
  }

  h1 {
    margin: 0;
    font-size: var(--font-size-lg);
  }

  h2 {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    margin: 0;
    font-size: var(--font-size-sm);
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--color-text-muted);
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

  .monitor {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }

  .monitor .warn {
    flex-basis: 100%;
    color: var(--color-warning);
  }

  /* The strip: eight modules side by side on one rail. */
  .stages {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(104px, 1fr));
    gap: var(--space-1);
    list-style: none;
    margin: 0;
    padding: var(--space-1);
    background: var(--color-surface-sunken);
    border: 1px solid var(--color-panel-edge);
    border-radius: var(--radius-md);
  }

  .stage {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    width: 100%;
    height: 100%;
    padding: var(--space-2);
    text-align: left;
    background: linear-gradient(var(--color-panel), var(--color-surface));
    border: 1px solid var(--color-panel-edge);
    border-radius: var(--radius-sm);
  }

  .stage[aria-expanded='true'] {
    border-color: var(--color-accent);
    box-shadow: inset 0 0 0 1px var(--color-accent);
  }

  .stage .head {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    min-width: 0;
  }

  .led {
    flex: none;
    width: 7px;
    height: 7px;
    border-radius: var(--radius-full);
    background: var(--color-state-offline);
    transition: background var(--duration-fast) var(--ease-standard);
  }

  .led.lit {
    background: var(--color-speaking);
    box-shadow: 0 0 6px var(--color-speaking);
  }

  .name {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: var(--font-size-xs);
    font-weight: 600;
  }

  .state,
  .muted {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .state {
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
  }

  .muted {
    margin: 0;
  }

  /* The unit: one rack panel for the open stage. */
  .unit {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4) var(--space-4);
    background: linear-gradient(var(--color-panel), var(--color-surface));
    border: 1px solid var(--color-panel-edge);
    border-radius: var(--radius-md);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.05),
      0 2px 6px rgba(0, 0, 0, 0.25);
  }

  .unit .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
    min-height: 28px;
    padding-bottom: var(--space-2);
    border-bottom: 1px solid var(--color-panel-edge);
  }

  .knobs {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: var(--space-3) var(--space-2);
  }

  .knobs .hint {
    flex: 1 1 200px;
    align-self: center;
    max-width: 36ch;
  }

  label {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .check,
  .power {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--color-text);
  }

  .power {
    padding: var(--space-1) var(--space-2);
    font-size: var(--font-size-sm);
    background: var(--color-surface-sunken);
    border: 1px solid var(--color-knob-edge);
    border-radius: var(--radius-sm);
    cursor: pointer;
  }

  input {
    accent-color: var(--color-accent);
  }

  .band {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin: 0;
    border: 1px solid var(--color-panel-edge);
    border-radius: var(--radius-sm);
    padding: var(--space-2) var(--space-3) var(--space-3);
  }

  .band legend {
    padding: 0 var(--space-1);
    font-size: var(--font-size-xs);
    font-weight: 600;
    color: var(--color-text-muted);
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
