<script lang="ts">
  // Inspector „Soundboard“ (ADR-010): the sound folder as buttons. A click plays the sound
  // for everybody in the channel on my own soundboard track — and for me, so I hear what
  // they get. The folder is the library: „Sound hinzufügen“ copies a file in, „Ordner
  // öffnen“ shows it. No hotkeys yet (that is the follow-up).
  import { FolderOpen, Plus, Square } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import { getApp } from '../../app.svelte';
  import Info from '../../Info.svelte';

  const { session } = getApp();

  let query = $state('');
  let picker = $state<HTMLInputElement | null>(null);
  const gain = $derived(session.settings?.soundboardGainDb ?? 0);
  const shown = $derived(
    session.sounds.filter((s) => s.name.toLowerCase().includes(query.trim().toLowerCase())),
  );

  onMount(() => {
    void session.loadSounds();
  });

  async function picked(event: Event & { currentTarget: HTMLInputElement }) {
    // `currentTarget` is gone after the first await: keep the element.
    const input = event.currentTarget;
    const files = Array.from(input.files ?? []);
    for (const file of files) await session.addSound(file);
    input.value = '';
  }
</script>

<section class="soundboard" aria-labelledby="soundboard-title">
  <h2 id="soundboard-title">Soundboard <Info term="soundboard" /></h2>
  <p class="hint">
    Ein Klick spielt den Sound für alle im Channel – über deinen eigenen Soundboard-Kanal, den jeder
    für sich leiser drehen kann.
  </p>

  <div class="tools">
    <input type="search" placeholder="Suchen …" aria-label="Sounds suchen" bind:value={query} />
    <button
      class="small"
      disabled={session.playing.length === 0}
      onclick={() => session.stopSounds()}
    >
      <Square size={14} aria-hidden="true" />
      Stopp
    </button>
  </div>

  {#if session.sounds.length === 0}
    <p class="empty">
      Noch keine Sounds. Lege Dateien in den Ordner (WAV, MP3, OGG, FLAC, M4A) oder füge eine hinzu.
    </p>
  {:else if shown.length === 0}
    <p class="empty">Kein Sound heißt so.</p>
  {:else}
    <ul class="grid" aria-label="Sounds">
      {#each shown as sound (sound.path)}
        <li>
          <button
            class="pad"
            class:playing={session.isPlaying(sound.path)}
            aria-pressed={session.isPlaying(sound.path)}
            title={sound.path}
            onclick={() => session.playSound(sound.path)}
          >
            {sound.name}
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  <label for="soundboard-gain">Lautstärke meiner Sounds</label>
  <div class="row">
    <input
      id="soundboard-gain"
      type="range"
      min="-30"
      max="12"
      step="1"
      value={gain}
      oninput={(e) => session.applySettings({ soundboardGainDb: Number(e.currentTarget.value) })}
    />
    <span class="db">{gain > 0 ? '+' : ''}{gain} dB</span>
  </div>

  <div class="actions">
    <button class="small" onclick={() => picker?.click()}>
      <Plus size={14} aria-hidden="true" />
      Sound hinzufügen
    </button>
    <button class="small" onclick={() => session.openSoundFolder()}>
      <FolderOpen size={14} aria-hidden="true" />
      Ordner öffnen
    </button>
    <input
      bind:this={picker}
      type="file"
      accept=".wav,.flac,.ogg,.oga,.mp3,.m4a,.aac,audio/*"
      multiple
      hidden
      aria-label="Sounddatei wählen"
      onchange={picked}
    />
  </div>
  {#if session.soundProblem}
    <p class="problem" role="alert">{session.soundProblem}</p>
  {/if}
</section>

<style>
  h2 {
    margin: 0 var(--space-6) var(--space-1) 0;
    font-size: var(--font-size-lg);
  }

  .hint,
  .empty,
  label {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .hint {
    margin: 0 0 var(--space-3);
  }

  .tools,
  .actions,
  .row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .tools {
    margin-bottom: var(--space-3);
  }

  .tools input {
    flex: 1;
    min-width: 0;
  }

  .tools button,
  .actions button {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    white-space: nowrap;
  }

  .grid {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(96px, 1fr));
    gap: var(--space-2);
  }

  .pad {
    width: 100%;
    min-height: 56px;
    padding: var(--space-2);
    overflow-wrap: anywhere;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-surface-raised, var(--color-surface));
  }

  .pad.playing {
    border-color: var(--color-accent);
    box-shadow: 0 0 0 2px var(--color-accent);
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

  .actions {
    flex-wrap: wrap;
  }

  .problem {
    color: var(--color-danger);
    font-size: var(--font-size-sm);
  }
</style>
