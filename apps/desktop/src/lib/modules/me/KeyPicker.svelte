<script lang="ts">
  // One global key (push-to-talk, mute or deafen): shown by name, changed by pressing the
  // new one, removed with a click. The hook is global, so the key also works while a game
  // has the focus (S3). One key does one thing — the shell refuses a key another action
  // already has, and the refusal is shown here.
  import { onMount } from 'svelte';
  import { getApp } from '../../app.svelte';
  import { keyLabel } from '../../keys';
  import type { ApiError } from '../../types/ApiError';
  import type { AppSettings } from '../../types/AppSettings';
  import type { KeyAction } from '../../types/KeyAction';

  interface Props {
    action: KeyAction;
    /** Shown when no key is set; `undefined` = a missing key is fine. */
    warnWithout?: string;
    note?: string;
  }

  const { action, warnWithout, note }: Props = $props();

  const FIELD: Record<KeyAction, keyof AppSettings & ('pttKey' | 'muteKey' | 'deafenKey')> = {
    ptt: 'pttKey',
    mute: 'muteKey',
    deafen: 'deafenKey',
  };

  const app = getApp();
  const { api } = app;
  let key = $state<string | null>(null);
  let waiting = $state(false);
  let global = $state(true);
  let problem = $state<string | null>(null);

  onMount(() => {
    void api.appSettings().then((s) => (key = s[FIELD[action]]));
    void api.hotkeysGlobal().then((g) => (global = g));
  });

  async function change() {
    waiting = true;
    problem = null;
    try {
      const pressed = await api.captureKey(action);
      if (pressed) key = pressed;
    } catch (reason) {
      const error = reason as Partial<ApiError>;
      problem = error.message ?? 'Die Taste konnte nicht gesetzt werden.';
    } finally {
      waiting = false;
    }
  }

  async function remove() {
    problem = null;
    await app.updateAppSettings({ [FIELD[action]]: null });
    key = null;
  }
</script>

<!-- Escape cancels the capture (in Rust); it must not also step the shell back. -->
<svelte:window
  onkeydowncapture={(e) => {
    if (waiting && e.key === 'Escape') e.preventDefault();
  }}
/>

<div class="picker">
  <p class="current">
    Taste: <strong>{keyLabel(key)}</strong>
  </p>
  <div class="row">
    <button onclick={change} disabled={waiting} aria-live="polite">
      {waiting
        ? 'Drück jetzt die Taste … (Esc bricht ab)'
        : key
          ? 'Andere Taste'
          : 'Taste festlegen'}
    </button>
    {#if key && !waiting}
      <button class="quiet" onclick={remove}>Entfernen</button>
    {/if}
  </div>
  {#if problem}
    <p class="warn" role="alert">{problem}</p>
  {/if}
  {#if !key && warnWithout}
    <p class="warn" role="status">{warnWithout}</p>
  {/if}
  {#if !global}
    <p class="warn" role="status">
      Die Taste wirkt nur, solange yAPPA im Vordergrund ist (keine Erlaubnis für globale Tasten).
    </p>
  {/if}
  {#if note}
    <p class="note">{note}</p>
  {/if}
</div>

<style>
  .picker {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-2);
    margin-top: var(--space-2);
  }

  .row {
    display: flex;
    gap: var(--space-2);
  }

  .quiet {
    color: var(--color-text-muted);
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
