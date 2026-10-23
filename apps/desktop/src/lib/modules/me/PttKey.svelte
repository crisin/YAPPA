<script lang="ts">
  // The push-to-talk key: shown by name, changed by pressing the new one. The hook is
  // global, so the key also works while a game has the focus (S3).
  import { onMount } from 'svelte';
  import { getApp } from '../../app.svelte';
  import { keyLabel } from '../../keys';

  const { api } = getApp();
  let key = $state<string | null>(null);
  let waiting = $state(false);
  let global = $state(true);

  onMount(() => {
    void api.appSettings().then((s) => (key = s.pttKey));
    void api.hotkeysGlobal().then((g) => (global = g));
  });

  async function change() {
    waiting = true;
    try {
      const pressed = await api.capturePttKey();
      if (pressed) key = pressed;
    } finally {
      waiting = false;
    }
  }
</script>

<!-- Escape cancels the capture (in Rust); it must not also step the shell back. -->
<svelte:window
  onkeydowncapture={(e) => {
    if (waiting && e.key === 'Escape') e.preventDefault();
  }}
/>

<div class="ptt">
  <p class="current">
    Taste: <strong>{keyLabel(key)}</strong>
  </p>
  <button onclick={change} disabled={waiting} aria-live="polite">
    {waiting ? 'Drück jetzt die Taste … (Esc bricht ab)' : key ? 'Andere Taste' : 'Taste festlegen'}
  </button>
  {#if !key}
    <p class="warn" role="status">Ohne Taste geht dein Mikro in diesem Modus nie an.</p>
  {/if}
  {#if !global}
    <p class="warn" role="status">
      Die Taste wirkt nur, solange yAPPA im Vordergrund ist (keine Erlaubnis für globale Tasten).
    </p>
  {/if}
  <p class="note">
    Gut sind Tasten, die das Spiel nicht braucht – zum Beispiel eine Seitentaste der Maus.
  </p>
</div>

<style>
  .ptt {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-2);
    margin-top: var(--space-2);
  }

  .current {
    margin: 0;
  }

  .warn {
    margin: 0;
    color: var(--color-warning);
    font-size: var(--font-size-sm);
  }

  .note {
    margin: 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }
</style>
