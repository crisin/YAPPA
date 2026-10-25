import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Engine } from './engine';
import { createMockEngine, encodeInvite } from './mockEngine';
import { LOG_LIMIT, Session, meterFill } from './session.svelte';
import type { Command } from './types/Command';

const invite = encodeInvite({ url: 'wss://voice.example', token: 'tok' });

/** A mock engine that also records what the UI sent. */
function recording(): { engine: Engine; sent: Command[] } {
  const mock = createMockEngine();
  const sent: Command[] = [];
  const engine: Engine = {
    ...mock,
    send(command) {
      sent.push(command);
      return mock.send(command);
    },
  };
  return { engine, sent };
}

beforeEach(() => localStorage.clear());

describe('Session', () => {
  it('joins with the decoded invite and remembers the text', async () => {
    const { engine, sent } = recording();
    const session = new Session(engine);
    await session.start();
    session.invite = `  ${invite}\n`;
    await session.join();

    expect(sent).toContainEqual({ type: 'join', url: 'wss://voice.example', token: 'tok' });
    expect(session.connection).toBe('connected');
    expect(session.room).toBe('gang');
    expect(session.participants.map((p) => p.name)).toEqual(['Anna', 'Ben']);
    expect(new Session(engine).invite).toBe(invite);
  });

  it('explains a bad invite in German and sends nothing', async () => {
    const { engine, sent } = recording();
    const session = new Session(engine);
    session.invite = 'irgendein Text';
    await session.join();
    expect(session.inviteProblem).toMatch(/keine yAPPA-Einladung/);

    session.invite = invite.slice(0, -4);
    await session.join();
    expect(session.inviteProblem).toMatch(/unvollständig/);
    expect(sent).toEqual([]);
    expect(localStorage.length).toBe(0);
  });

  it('keeps the send chain stage by stage for the pro view', () => {
    const session = new Session(createMockEngine());
    session.apply({
      type: 'stageLevels',
      stages: [{ id: 'gate', levelDb: -30, active: false }],
    });
    expect(session.stages).toEqual([{ id: 'gate', levelDb: -30, active: false }]);
  });

  it('forgets room and levels when disconnected, keeps them while reconnecting', () => {
    const session = new Session(createMockEngine());
    session.apply({ type: 'joined', room: 'gang', identity: 'ich' });
    session.apply({ type: 'connection', state: 'connected' });
    session.apply({
      type: 'levels',
      inputDb: -30,
      gateOpen: true,
      peers: [{ identity: 'anna', levelDb: -20, speaking: true }],
    });
    expect(session.peerLevels.anna.speaking).toBe(true);

    session.apply({ type: 'connection', state: 'reconnecting' });
    expect(session.inRoom).toBe(true);
    expect(session.room).toBe('gang');

    session.apply({ type: 'connection', state: 'disconnected' });
    expect(session.inRoom).toBe(false);
    expect(session.room).toBeNull();
    expect(session.peerLevels).toEqual({});
  });

  it('shows an engine error until the next successful join', () => {
    const session = new Session(createMockEngine());
    session.apply({ type: 'error', message: 'could not connect' });
    expect(session.error).toBe('could not connect');
    session.apply({ type: 'joined', room: 'gang', identity: 'ich' });
    expect(session.error).toBeNull();
  });

  it('says in German which device stands in, and nothing once the chosen one is back', () => {
    const session = new Session(createMockEngine());
    expect(session.deviceNote).toBeNull();
    session.apply({
      type: 'deviceState',
      input: { open: 'Mikrofon (Realtek)', fallback: true, missing: 'Headset-Mikrofon' },
      output: { open: 'Kopfhörer', fallback: false, missing: null },
    });
    expect(session.deviceNote).toBe(
      'Mikrofon „Headset-Mikrofon“ ist gerade nicht angeschlossen – solange läuft „Mikrofon (Realtek)“. Es kommt von selbst zurück, sobald es wieder da ist.',
    );
    // Never seen before (chosen on an earlier start): no name, and nothing else to open.
    session.apply({
      type: 'deviceState',
      input: { open: 'Headset-Mikrofon', fallback: false, missing: null },
      output: { open: null, fallback: true, missing: null },
    });
    expect(session.deviceNote).toMatch(/^Dein gewähltes Ausgabe-Gerät .* keine andere Ausgabe/);
    session.apply({
      type: 'deviceState',
      input: { open: 'Headset-Mikrofon', fallback: false, missing: null },
      output: { open: 'Kopfhörer', fallback: false, missing: null },
    });
    expect(session.deviceNote).toBeNull();
  });

  it('keeps the newest log entries only', () => {
    const session = new Session(createMockEngine());
    for (let n = 0; n < LOG_LIMIT + 5; n += 1) {
      session.apply({
        type: 'log',
        entry: { timeMs: n, level: 'info', target: 't', message: `m${n}` },
      });
    }
    expect(session.log).toHaveLength(LOG_LIMIT);
    expect(session.log.at(-1)?.message).toBe(`m${LOG_LIMIT + 4}`);
  });

  it('turns mute, deafen, volume and device choices into commands', async () => {
    const { engine, sent } = recording();
    const session = new Session(engine);
    await session.start();
    await session.toggleMute();
    await session.toggleDeafen();
    await session.setPeerGain('anna', -6);
    await session.togglePeerMute('anna');
    await session.setInput('mic-2');
    await session.setOutput(null);

    expect(sent).toEqual([
      { type: 'toggleMuted' },
      { type: 'toggleDeafened' },
      { type: 'setPeerVolume', identity: 'anna', track: 'voice', gainDb: -6 },
      { type: 'setPeerMuted', identity: 'anna', track: 'voice', muted: true },
      { type: 'setInputDevice', id: 'mic-2' },
      { type: 'setOutputDevice', id: null },
    ]);
    expect(session.settings?.inputDevice).toBe('mic-2');
  });

  it('follows the engine for mute and deafen: deafen implies mute, undeafen restores', async () => {
    const session = new Session(createMockEngine());
    const stop = await session.start();
    expect([session.muted, session.deafened]).toEqual([false, false]);
    await session.toggleDeafen();
    expect([session.muted, session.deafened]).toEqual([true, true]);
    await session.toggleDeafen();
    expect([session.muted, session.deafened]).toEqual([false, false]);
    await session.toggleMute();
    await session.toggleDeafen();
    await session.toggleDeafen();
    expect([session.muted, session.deafened]).toEqual([true, false]);
    // A key press in a game reaches the UI the same way: as the engine's event.
    session.apply({ type: 'muteState', muted: false, deafened: false });
    expect(session.muted).toBe(false);
    stop();
  });

  it('receives levels and stats from the running engine', async () => {
    vi.useFakeTimers();
    const session = new Session(createMockEngine());
    const stop = await session.start();
    session.invite = invite;
    await session.join();
    await vi.advanceTimersByTimeAsync(1100);
    expect(session.stats?.network?.rttMs).toBe(18);
    expect(session.inputs).toHaveLength(2);
    expect(Object.keys(session.peerLevels)).toEqual(['anna']);
    stop();
    vi.useRealTimers();
  });
});

describe('meterFill', () => {
  it('maps -60 dBFS..0 to an empty..full bar', () => {
    expect(meterFill(-120)).toBe(0);
    expect(meterFill(-60)).toBe(0);
    expect(meterFill(-30)).toBe(0.5);
    expect(meterFill(0)).toBe(1);
    expect(meterFill(6)).toBe(1);
  });
});
