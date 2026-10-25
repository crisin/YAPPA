// A stand-in control plane and presence room for the browser and for tests: the fixture
// "Die Gang" from the UI concept, arranged per scene. Joining a channel drives the engine
// mock like the real client drives the engine. Admin actions change the fixture in memory
// so the admin screens can be clicked through. No network.
import type { Api, PresenceEntry } from './api';
import type { Engine } from './engine';
import type { ApiError } from './types/ApiError';
import type { AppSettings } from './types/AppSettings';
import type { Channel } from './types/Channel';
import defaultAppSettings from './types/defaultAppSettings.json';
import type { EventEntry } from './types/EventEntry';
import type { InviteCode } from './types/InviteCode';
import type { KeyAction } from './types/KeyAction';
import type { Member } from './types/Member';
import type { Participant } from './types/Participant';
import type { PresenceState } from './types/PresenceState';
import type { PublicConfig } from './types/PublicConfig';
import type { Role } from './types/Role';
import type { UpdateStatus } from './types/UpdateStatus';
import type { User } from './types/User';
import { DEFAULT_SCENE, type Scene } from './scene';

const user = (id: string, displayName: string, role: Role = 'member'): User => ({
  id,
  displayName,
  role,
});

/** The gang of the UI concept. Krille is the one at the keyboard in most scenes. */
export const GANG: readonly User[] = [
  user('krille', 'Krille', 'admin'),
  user('ben', 'Ben'),
  user('sasha', 'Sasha'),
  user('tim', 'Tim'),
  user('lea', 'Lea'),
  user('mo', 'Mo'),
  user('jule', 'Jule'),
  user('rico', 'Rico'),
];

const SERVER = 'home';

export const CHANNELS: readonly Channel[] = [
  { id: 'lobby', name: 'Lobby', position: 0, serverId: SERVER },
  { id: 'raid', name: 'Raid-Abend', position: 1, serverId: SERVER },
  { id: 'musik', name: 'Musik-Session', position: 2, serverId: SERVER },
  { id: 'afk', name: 'AFK', position: 3, serverId: SERVER },
];

export const MOCK_WS_URL = 'wss://voice.yappa.example';
export const MOCK_SERVER = 'https://voice.yappa.example/api';
/** A link the mock accepts once (scene `login`). */
export const MOCK_INVITE_LINK = `yappa://invite/gang-7f3k?server=${MOCK_SERVER}`;

const state = (channelId: string | null, extra: Partial<PresenceState> = {}): PresenceState => ({
  channelId,
  app: 'nightly 0.1.0',
  talk: 'vad',
  away: false,
  muted: false,
  deafened: false,
  ...extra,
});

/** Who is where in the busy scenes (concept screen "Im Call"); I start nowhere. */
const EVENING: Record<string, PresenceState> = {
  ben: state('raid'),
  sasha: state('raid'),
  tim: state('raid', { muted: true }),
  lea: state('raid', { talk: 'ptt' }),
  mo: state('lobby'),
  jule: state('lobby'),
  rico: state('afk', { away: true, app: 'stable 0.0.9' }),
};

const QUIET: Record<string, PresenceState> = {
  rico: state('afk', { away: true, app: 'stable 0.0.9' }),
};

/** Tim has muted himself in the evening scene (concept screen). */
const SELF_MUTED = new Set(['tim']);

/** Who the engine mock finds in a room, as LiveKit would list them. */
export function mockPeopleIn(room: string, me: string): Participant[] {
  return Object.entries(EVENING)
    .filter(([id, s]) => s.channelId === room && id !== me)
    .map(([id]) => ({
      identity: id,
      name: GANG.find((u) => u.id === id)?.displayName ?? id,
      hasVoice: true,
      muted: SELF_MUTED.has(id),
    }));
}

const reject = (code: string, message: string): Promise<never> =>
  Promise.reject<never>({ code, message } satisfies ApiError);

const UNREACHABLE: ApiError = {
  code: 'unreachable',
  message: 'Server nicht erreichbar: keine Verbindung zu „mock.yappa.test“. Server aus?',
};

/** A fixed clock so snapshots and tests do not drift. */
const T0 = Date.UTC(2026, 9, 23, 19, 0, 0);
const at = (minutes: number) => new Date(T0 + minutes * 60_000).toISOString();

/** The code of a `yappa://invite/<code>?server=…` link, or `null`. */
function linkCode(link: string): string | null {
  const match = /^yappa:\/\/invite\/([a-z0-9-]+)\?server=https?:\/\/\S+$/i.exec(
    link.replace(/\s+/g, ''),
  );
  return match ? match[1].toLowerCase() : null;
}

/** `engine` (optional) is driven on join/leave, as the real client drives the engine. */
export function createMockApi(scene: Scene = DEFAULT_SCENE, engine?: Engine): Api {
  const users = new Map(GANG.map((u) => [u.id, structuredClone(u)]));
  const channels: Channel[] = structuredClone(CHANNELS) as Channel[];
  const myId = scene === 'member' ? 'ben' : 'krille';
  const presence = new Map<string, PresenceState>(
    Object.entries(structuredClone(scene === 'lobby' ? QUIET : EVENING)),
  );
  let loggedIn = scene !== 'login' && scene !== 'invite';
  if (loggedIn) presence.set(myId, state(null));
  const handlers = new Set<(people: PresenceEntry[]) => void>();
  const invites: InviteCode[] = [
    {
      code: 'gang-7f3k',
      createdBy: 'krille',
      createdAt: at(-120),
      expiresAt: at(60 * 24 * 7),
      usedBy: null,
    },
  ];
  const events: EventEntry[] = [
    {
      time: at(-30),
      kind: 'joined',
      userId: 'ben',
      channelId: 'raid',
      message: 'Ben betritt raid',
    },
    {
      time: at(-29),
      kind: 'joined',
      userId: 'sasha',
      channelId: 'raid',
      message: 'Sasha betritt raid',
    },
    { time: at(-12), kind: 'online', userId: 'tim', channelId: null, message: 'Tim ist online' },
    {
      time: at(-3),
      kind: 'invite_redeemed',
      userId: 'lea',
      channelId: null,
      message: 'Lea ist dabei (0.1.0)',
    },
  ];
  let nextCode = 1;
  let appSettings: AppSettings = structuredClone(defaultAppSettings as AppSettings);

  /** Me first, then the others by name — the order crates/client hands out. */
  const snapshot = (): PresenceEntry[] => {
    const entry = ([userId, s]: [string, PresenceState]): PresenceEntry => ({
      userId,
      displayName: users.get(userId)?.displayName ?? userId,
      state: structuredClone(s),
    });
    const all = [...presence].map(entry);
    const mine = all.filter((e) => e.userId === myId);
    const others = all
      .filter((e) => e.userId !== myId)
      .sort((a, b) => a.displayName.localeCompare(b.displayName, 'de'));
    return [...mine, ...others];
  };
  const notify = () => {
    const people = snapshot();
    handlers.forEach((handler) => handler(people));
  };
  const online = <T>(value: () => T): Promise<T> =>
    scene === 'offline' ? Promise.reject(UNREACHABLE) : Promise.resolve(value());
  const me = () => users.get(myId)!;
  const mustBeAdmin = <T>(value: () => T): Promise<T> =>
    me().role === 'admin' ? online(value) : reject('forbidden', 'Nur für Admins.');
  const sorted = () => [...channels].sort((a, b) => a.position - b.position);
  const setMine = (patch: Partial<PresenceState>) => {
    presence.set(myId, { ...(presence.get(myId) ?? state(null)), ...patch });
    notify();
  };

  // The versions the mock engine reports 0.1.0; these two scenes offer a newer one.
  const update: UpdateStatus =
    scene === 'update'
      ? { kind: 'ready', version: '99.0.0', notes: '- Alles neu.' }
      : scene === 'update-ready'
        ? { kind: 'ready', version: '0.2.0', notes: '- Updates installieren sich selbst (D1).' }
        : { kind: 'upToDate' };
  const config: PublicConfig = {
    apiVersion: 1,
    minVersion: scene === 'update' ? '99.0.0' : '0.1.0',
    killedVersions: [],
    updateBaseUrl: 'https://yappa.example/updates',
    livekitWsUrl: MOCK_WS_URL,
  };

  return {
    resume: () =>
      scene === 'offline'
        ? Promise.reject(UNREACHABLE)
        : Promise.resolve(loggedIn ? structuredClone(me()) : null),
    redeem(link, displayName) {
      if (scene === 'offline') return Promise.reject(UNREACHABLE);
      const code = linkCode(link);
      if (!code) return reject('invite_invalid', 'Das ist kein yAPPA-Einladungslink.');
      const invite = invites.find((i) => i.code === code && i.usedBy === null);
      if (!invite)
        return reject(
          'invite_invalid',
          'Diese Einladung ist ungültig, abgelaufen oder schon benutzt.',
        );
      const name = displayName.trim().replace(/\s+/g, ' ');
      if (name.length < 2 || name.length > 32)
        return reject('name_invalid', 'Der Name braucht 2 bis 32 Zeichen.');
      invite.usedBy = myId;
      users.set(myId, { ...me(), displayName: name });
      loggedIn = true;
      events.push({
        time: at(0),
        kind: 'invite_redeemed',
        userId: myId,
        channelId: null,
        message: `${name} ist dabei (0.1.0)`,
      });
      setMine({});
      return Promise.resolve(structuredClone(me()));
    },
    async logout() {
      await engine?.send({ type: 'leave' });
      loggedIn = false;
      presence.delete(myId);
      notify();
    },
    config: () => online(() => structuredClone(config)) as Promise<PublicConfig>,
    channels: () => online(() => structuredClone(sorted())),
    presence(handler) {
      handlers.add(handler);
      handler(snapshot());
      return Promise.resolve(() => {
        handlers.delete(handler);
      });
    },
    setPresence(patch) {
      if (scene === 'offline') return Promise.reject(UNREACHABLE);
      setMine(patch);
      return Promise.resolve();
    },
    async joinChannel(channelId) {
      if (scene === 'offline') throw UNREACHABLE;
      if (!channels.some((c) => c.id === channelId))
        throw {
          code: 'channel_unknown',
          message: 'Diesen Channel gibt es nicht.',
        } satisfies ApiError;
      await engine?.send({ type: 'join', url: MOCK_WS_URL, token: `mock.${channelId}.${myId}` });
      setMine({ channelId });
    },
    async leaveChannel() {
      await engine?.send({ type: 'leave' });
      setMine({ channelId: null });
    },
    appSettings: () => Promise.resolve(structuredClone(appSettings)),
    setAppSettings(settings) {
      appSettings = structuredClone(settings);
      return Promise.resolve(structuredClone(appSettings));
    },
    // In the browser there is no hook: pretend a key was pressed — the mouse side button
    // for push-to-talk, F7 and F8 for mute and deafen. One key does one thing, as in Rust.
    captureKey(action) {
      const field = { ptt: 'pttKey', mute: 'muteKey', deafen: 'deafenKey' } as const;
      const pressed = { ptt: 'Mouse4', mute: 'F7', deafen: 'F8' }[action];
      const holder = (Object.keys(field) as KeyAction[]).find(
        (a) => a !== action && appSettings[field[a]] === pressed,
      );
      if (holder) {
        const label = { ptt: 'Push-to-Talk', mute: 'Stumm', deafen: 'Taub' }[holder];
        return reject('key_taken', `Diese Taste ist schon für „${label}“ belegt.`);
      }
      appSettings = { ...appSettings, [field[action]]: pressed };
      return Promise.resolve(pressed);
    },
    hotkeysGlobal: () => Promise.resolve(true),
    checkUpdate: () =>
      scene === 'offline'
        ? Promise.resolve({ kind: 'failed', reason: 'Server nicht erreichbar.' })
        : Promise.resolve(structuredClone(update)),
    installUpdate: () =>
      update.kind === 'ready'
        ? Promise.resolve()
        : reject('no_update', 'Es ist kein Update heruntergeladen.'),
    inviteLinks(handler) {
      // Scene `invite`: the app was started by clicking the invite link.
      if (scene === 'invite') queueMicrotask(() => handler(MOCK_INVITE_LINK));
      return Promise.resolve(() => {});
    },
    admin: {
      members: () =>
        mustBeAdmin(() =>
          [...users.values()].map((u): Member => ({
            user: structuredClone(u),
            client: presence.has(u.id)
              ? {
                  appVersion: presence.get(u.id)!.app.split(' ')[1] ?? '0.0.0',
                  protocolVersion: 2,
                  platform: 'windows',
                }
              : null,
            lastSeen: presence.has(u.id) ? at(0) : at(-60 * 24 * 3),
          })),
        ),
      setRole: (userId, role) =>
        mustBeAdmin(() => {
          const target = users.get(userId);
          if (!target)
            throw { code: 'user_unknown', message: 'Unbekannte Person.' } satisfies ApiError;
          target.role = role;
        }),
      kick: (userId) =>
        mustBeAdmin(() => {
          presence.delete(userId);
          events.push({
            time: at(0),
            kind: 'kicked',
            userId,
            channelId: null,
            message: `${users.get(userId)?.displayName ?? userId} wurde rausgeworfen`,
          });
          notify();
        }),
      invites: () => mustBeAdmin(() => structuredClone(invites)),
      createInvite: () =>
        mustBeAdmin(() => {
          const invite: InviteCode = {
            code: `gang-${String(nextCode++).padStart(4, '0')}`,
            createdBy: myId,
            createdAt: at(0),
            expiresAt: at(60 * 24 * 7),
            usedBy: null,
          };
          invites.push(invite);
          return structuredClone(invite);
        }),
      inviteLink: (code) => mustBeAdmin(() => `yappa://invite/${code}?server=${MOCK_SERVER}`),
      revokeInvite: (code) =>
        mustBeAdmin(() => {
          const index = invites.findIndex((i) => i.code === code);
          if (index >= 0) invites.splice(index, 1);
        }),
      createChannel: (name) =>
        mustBeAdmin(() => {
          const channel: Channel = {
            id:
              name
                .toLowerCase()
                .replace(/[^a-z0-9]+/g, '-')
                .replace(/^-|-$/g, '') || `channel-${channels.length}`,
            name,
            position: channels.length,
            serverId: SERVER,
          };
          channels.push(channel);
          return structuredClone(channel);
        }),
      renameChannel: (id, name) =>
        mustBeAdmin(() => {
          const channel = channels.find((c) => c.id === id);
          if (channel) channel.name = name;
        }),
      deleteChannel: (id) =>
        mustBeAdmin(() => {
          const index = channels.findIndex((c) => c.id === id);
          if (index >= 0) channels.splice(index, 1);
          sorted().forEach((c, position) => (c.position = position));
          for (const [userId, s] of presence) {
            if (s.channelId === id) presence.set(userId, { ...s, channelId: null });
          }
          notify();
        }),
      moveChannel: (id, position) =>
        mustBeAdmin(() => {
          const order = sorted().filter((c) => c.id !== id);
          const moving = channels.find((c) => c.id === id);
          if (!moving) return;
          order.splice(Math.max(0, Math.min(position, order.length)), 0, moving);
          order.forEach((c, index) => (c.position = index));
        }),
      events: () => mustBeAdmin(() => structuredClone(events).reverse()),
      setPolicy: (policy) =>
        mustBeAdmin(() => {
          config.minVersion = policy.minVersion;
          config.killedVersions = [...policy.killedVersions];
          return structuredClone(config);
        }),
    },
  };
}
