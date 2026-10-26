<script lang="ts">
  // Inspector „App-Audio“ (ADR-011): the running applications with sound, one click streams
  // one of them into the channel on my own `music` track — the others can turn it down per
  // person; I keep hearing the application as before, nothing is doubled. „Stopp“ ends it,
  // the slider sets how loud it goes out. Only shown where the OS can capture an
  // application (Windows 11 today).
  import { RefreshCw, Square } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import { getApp } from '../../app.svelte';
  import Info from '../../Info.svelte';

  const { session } = getApp();

  const gain = $derived(session.settings?.appAudioGainDb ?? 0);
  const running = $derived(session.appAudio.running);

  onMount(() => {
    void session.listApps();
  });
</script>

<section class="appaudio" aria-labelledby="appaudio-title">
  <h2 id="appaudio-title">App-Audio <Info term="app-audio" /></h2>
  <p class="hint">
    Streamt den Ton eines Programms – eines Spiels, eines Players, eines Browser-Tabs – in deinen
    Channel, über einen eigenen Kanal von dir, den jeder für sich leiser drehen kann. Du hörst das
    Programm weiter wie bisher.
  </p>

  {#if running}
    <p class="running" role="status">
      Streamt <strong>{session.appAudio.app}</strong>
    </p>
    <button class="small" onclick={() => session.stopAppAudio()}>
      <Square size={14} aria-hidden="true" />
      Stopp
    </button>
  {/if}

  <div class="tools">
    <p class="heading" id="apps-heading">Programme mit Ton</p>
    <button class="small" onclick={() => session.listApps()}>
      <RefreshCw size={14} aria-hidden="true" />
      Neu einlesen
    </button>
  </div>
  {#if session.apps.length === 0}
    <p class="empty">
      Kein Programm spielt gerade Ton – oder hat noch keinen gespielt. Starte den Ton im Programm
      und lies neu ein.
    </p>
  {:else}
    <ul class="apps" aria-labelledby="apps-heading">
      {#each session.apps as app (app.pid)}
        {@const mine = running && session.appAudio.app === app.name}
        <li>
          <button
            class="app"
            class:streaming={mine}
            aria-pressed={mine}
            title="PID {app.pid}"
            onclick={() => session.startAppAudio(app)}
          >
            <span class="name">{app.name}</span>
            <span class="state">{mine ? 'streamt' : app.active ? 'spielt gerade' : 'still'}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  <label for="appaudio-gain">Lautstärke meines App-Audios</label>
  <div class="row">
    <input
      id="appaudio-gain"
      type="range"
      min="-30"
      max="12"
      step="1"
      value={gain}
      oninput={(e) => session.applySettings({ appAudioGainDb: Number(e.currentTarget.value) })}
    />
    <span class="db">{gain > 0 ? '+' : ''}{gain} dB</span>
  </div>
  <p class="hint">
    Ein Limiter hält den Stream unter dem Maximum. Die anderen hören ihn in Mono, mit ihrem eigenen
    Regler bei deiner Person.
  </p>
</section>

<style>
  h2 {
    margin: 0 var(--space-6) var(--space-1) 0;
    font-size: var(--font-size-lg);
  }

  .hint,
  .empty,
  label,
  .heading {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .hint {
    margin: 0 0 var(--space-3);
  }

  .running {
    margin: 0 0 var(--space-2);
    color: var(--color-accent);
  }

  .tools,
  .row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .tools {
    justify-content: space-between;
    margin: var(--space-4) 0 var(--space-2);
  }

  .heading {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .tools button,
  .small {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    white-space: nowrap;
  }

  .apps {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .app {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: var(--space-2);
    width: 100%;
    padding: var(--space-2);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-surface-raised, var(--color-surface));
    text-align: left;
  }

  .app.streaming {
    border-color: var(--color-accent);
    box-shadow: 0 0 0 2px var(--color-accent);
  }

  .app .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }

  .app .state {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    white-space: nowrap;
  }

  label {
    display: block;
    margin-top: var(--space-4);
  }

  .row {
    margin-bottom: var(--space-3);
  }

  .row input {
    flex: 1;
    accent-color: var(--color-accent);
  }

  .db {
    font-family: var(--font-mono);
    font-size: var(--font-size-sm);
  }
</style>
