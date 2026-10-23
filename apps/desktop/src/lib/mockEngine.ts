// A stand-in engine for the browser and for tests: it answers every command the way the
// real one would, with two invented people in the room. No audio, no network.
import type { Engine } from './engine';
import type { Command } from './types/Command';
import type { Event } from './types/Event';
import type { Invite } from './types/Invite';
import type { Participant } from './types/Participant';
import type { Settings } from './types/Settings';
import type { Stats } from './types/Stats';
import defaultSettings from './types/defaultSettings.json';
import { mockPeopleIn } from './mockApi';
import { DEFAULT_SCENE, type Scene } from './scene';

// Generated from `Settings::default()` in engine-protocol — the mock cannot drift from Rust.
const defaults = defaultSettings as Settings;

const others: Participant[] = [
  { identity: 'anna', name: 'Anna', hasVoice: true, muted: false },
  { identity: 'ben', name: 'Ben', hasVoice: true, muted: true },
];

const device = (name: string) => ({
  name,
  sampleRate: 48000,
  channels: 2,
  callbackFrames: 480,
  xruns: 0,
});

export const INVITE_PREFIX = 'yappa1.';

/** Same text format as `Invite::encode` in api-types. */
export function encodeInvite(invite: Invite): string {
  const base64 = btoa(JSON.stringify(invite));
  return INVITE_PREFIX + base64.replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

function decodeInvite(text: string): Invite {
  const compact = text.replace(/\s+/g, '');
  if (!compact.startsWith(INVITE_PREFIX)) throw 'not a yAPPA invite (it starts with "yappa1.")';
  try {
    const base64 = compact.slice(INVITE_PREFIX.length).replace(/-/g, '+').replace(/_/g, '/');
    const invite = JSON.parse(atob(base64)) as Invite;
    if (typeof invite.url !== 'string' || typeof invite.token !== 'string') throw 'shape';
    return invite;
  } catch {
    throw 'the invite is damaged or was copied incompletely';
  }
}

/**
 * `scene` (see `scene.ts`) picks the trouble: `reconnect` loses the link every few seconds,
 * `offline` cannot join at all. Every other scene behaves like a healthy room.
 */
export function createMockEngine(scene: Scene = DEFAULT_SCENE): Engine {
  const handlers = new Set<(event: Event) => void>();
  const settings: Settings = structuredClone(defaults);
  let connected = false;
  /** Who is in the room now; the first one talks in waves. */
  let people: Participant[] = [];
  let muted = false;
  let ticks = 0;
  let timer: ReturnType<typeof setInterval> | undefined;

  const emit = (event: Event) => handlers.forEach((handler) => handler(event));
  const log = (message: string) =>
    emit({
      type: 'log',
      entry: { timeMs: Date.now(), level: 'info', target: 'mock', message },
    });

  const devices = (): Event => ({
    type: 'devices',
    inputs: [
      { id: 'mic-1', name: 'Headset-Mikrofon', isDefault: true },
      { id: 'mic-2', name: 'Webcam', isDefault: false },
    ],
    outputs: [{ id: 'out-1', name: 'Kopfhörer', isDefault: true }],
  });

  const stats = (): Stats => ({
    network: connected
      ? {
          rttMs: 18,
          sendCodec: 'audio/opus',
          sendKbpsRecent: 128,
          packetsSent: ticks * 5,
          uplinkLossPercent: 0,
          packetsReceived: ticks * 10,
          packetsLost: 0,
          lossPercentRecent: 0,
          jitterMs: 2,
          jitterBufferMs: 38,
          concealedPercentRecent: 0,
          concealedPercentTotal: 0,
        }
      : null,
    capture: device('Headset-Mikrofon'),
    playout: device('Kopfhörer'),
    sendIntervalMaxMs: 10.4,
    sendBlocksDropped: 0,
    playoutFramesDropped: 0,
    playoutSkips: 0,
  });

  // Ten times per second is enough to see the meters move.
  const tick = () => {
    ticks += 1;
    const wave = Math.sin(ticks / 4);
    emit({
      type: 'levels',
      inputDb: -38 + 10 * wave,
      gateOpen: !muted,
      peers:
        connected && people.length > 0
          ? [{ identity: people[0].identity, levelDb: -30 + 12 * wave, speaking: wave > 0 }]
          : [],
    });
    if (ticks % 10 === 0) emit({ type: 'stats', stats: stats() });
    // A flaky link: gone for two seconds out of every five.
    if (scene === 'reconnect' && connected) {
      if (ticks % 50 === 30) emit({ type: 'connection', state: 'reconnecting' });
      if (ticks % 50 === 0) emit({ type: 'connection', state: 'connected' });
    }
  };

  const handle = (command: Command) => {
    switch (command.type) {
      case 'join':
        emit({ type: 'connection', state: 'connecting' });
        log(`joining ${command.url}`);
        if (scene === 'offline') {
          emit({ type: 'error', message: 'Der Server antwortet nicht.' });
          emit({ type: 'connection', state: 'disconnected' });
          break;
        }
        connected = true;
        {
          // `mock.<room>.<me>` comes from the mock API's joinChannel: that room, those
          // people. Any other token is a pasted developer invite: room `gang`.
          const [kind, room, me] = command.token.split('.');
          const fromApi = kind === 'mock' && room && me;
          people = fromApi ? mockPeopleIn(room, me) : structuredClone(others);
          emit({ type: 'joined', room: fromApi ? room : 'gang', identity: fromApi ? me : 'ich' });
          emit({ type: 'participants', participants: structuredClone(people) });
        }
        emit({ type: 'connection', state: 'connected' });
        break;
      case 'leave':
        connected = false;
        emit({ type: 'participants', participants: [] });
        emit({ type: 'connection', state: 'disconnected' });
        log('left');
        break;
      case 'setMuted':
        muted = command.muted;
        break;
      case 'setInputDevice':
        settings.inputDevice = command.id;
        break;
      case 'setOutputDevice':
        settings.outputDevice = command.id;
        break;
      case 'refreshDevices':
      case 'resync':
        emit(devices());
        break;
      default:
        break;
    }
  };

  return {
    info: () =>
      // The version the app would have; protocol 0 says "no real engine".
      Promise.resolve({ version: '0.1.0', protocolVersion: 0, logDir: '', debugPanel: false }),
    settings: () => Promise.resolve(structuredClone(settings)),
    send(command) {
      handle(command);
      return Promise.resolve();
    },
    subscribe(handler) {
      handlers.add(handler);
      timer ??= setInterval(tick, 100);
      return Promise.resolve(() => {
        handlers.delete(handler);
        if (handlers.size === 0) {
          clearInterval(timer);
          timer = undefined;
        }
      });
    },
    recentLogs() {
      emit(devices());
      return Promise.resolve([]);
    },
    decodeInvite(text) {
      try {
        return Promise.resolve(decodeInvite(text));
      } catch (reason) {
        return Promise.reject(reason);
      }
    },
    exportLogs: () => Promise.resolve('(Browser-Modus: keine Logdatei)'),
    openUrl(url) {
      // In a browser the browser is the system browser; in tests there is none.
      window.open?.(url, '_blank', 'noopener');
      return Promise.resolve();
    },
  };
}
