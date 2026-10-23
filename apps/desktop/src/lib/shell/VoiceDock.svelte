<script lang="ts">
  // Zone D: belongs to the shell, not to the voice module — the call goes on whatever module
  // is open, so status, own level, mute, deafen and leave are always one click away.
  import { HeadphoneOff, Headphones, Mic, MicOff, PhoneOff } from '@lucide/svelte';
  import { getApp } from '../app.svelte';
  import Meter from '../Meter.svelte';

  const app = getApp();
  const { session, gang } = app;
  const where = $derived(gang.channelName(session.room) ?? session.room);

  const status = $derived(
    {
      disconnected: 'Nicht verbunden',
      connecting: 'Verbinde …',
      connected: 'Verbunden',
      reconnecting: 'Verbindung verloren – verbinde neu …',
    }[session.connection],
  );
  const rtt = $derived(session.stats?.network?.rttMs ?? null);
  const quality = $derived(
    rtt === null ? null : rtt < 60 ? 'gut' : rtt < 150 ? 'mäßig' : 'schlecht',
  );
</script>

<section class="dock" class:warn={session.connection === 'reconnecting'} aria-label="Sprache">
  <div class="line">
    <span class="dot" class:on={session.connection === 'connected'} aria-hidden="true"></span>
    <span class="status" role="status">
      {status}{#if where && session.connection !== 'disconnected'}&nbsp;· {where}{/if}
    </span>
  </div>
  {#if rtt !== null && session.connection === 'connected'}
    <p class="detail">{Math.round(rtt)} ms · {quality}</p>
  {/if}
  <Meter db={session.inputDb} active={session.sending} label="Mikrofonpegel" />
  <div class="buttons">
    <button
      class="icon"
      aria-pressed={session.muted}
      aria-label="Mikrofon stummschalten"
      title={session.muted ? 'Mikrofon ist aus' : 'Mikrofon ist an'}
      onclick={() => session.toggleMute()}
    >
      {#if session.muted}<MicOff size={18} aria-hidden="true" />{:else}<Mic
          size={18}
          aria-hidden="true"
        />{/if}
    </button>
    <button
      class="icon"
      aria-pressed={session.deafened}
      aria-label="Ton stummschalten"
      title={session.deafened ? 'Du hörst nichts' : 'Du hörst alle'}
      onclick={() => session.toggleDeafen()}
    >
      {#if session.deafened}<HeadphoneOff size={18} aria-hidden="true" />{:else}<Headphones
          size={18}
          aria-hidden="true"
        />{/if}
    </button>
    {#if session.inRoom}
      <button class="leave" onclick={() => app.leaveVoice()}>
        <PhoneOff size={16} aria-hidden="true" />Verlassen
      </button>
    {/if}
  </div>
  <p class="detail">
    {session.muted
      ? 'Mikrofon aus'
      : session.sending
        ? 'Mikrofon an'
        : 'Mikrofon an, still'}{session.deafened ? ' · Ton aus' : ''}
  </p>
</section>

<style>
  .dock {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin: var(--space-2);
    padding: var(--space-3);
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
  }

  .dock.warn {
    border-color: var(--color-warning);
  }

  .line {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }

  .status {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .warn .status {
    color: var(--color-warning);
  }

  .dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: var(--radius-full);
    background: var(--color-state-offline);
  }

  .dot.on {
    background: var(--color-speaking);
  }

  .detail {
    margin: 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .buttons {
    display: flex;
    gap: var(--space-2);
  }

  .icon {
    display: grid;
    place-items: center;
    padding: var(--space-2);
  }

  .icon[aria-pressed='true'] {
    background: var(--color-danger);
    border-color: var(--color-danger);
    color: var(--color-accent-text);
  }

  .leave {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    margin-left: auto;
    border-color: var(--color-danger);
    color: var(--color-danger);
  }
</style>
