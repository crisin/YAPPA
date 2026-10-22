// A stand-in control plane and presence room for the browser and for tests: the fixture
// "Die Gang" from the UI concept, arranged per scene. Admin actions change the fixture in
// memory so the admin screens can be clicked through. No network.
import type { Api, PresenceMap } from './api';
import type { ApiError } from './types/ApiError';
import type { Channel } from './types/Channel';
import type { EventEntry } from './types/EventEntry';
import type { InviteCode } from './types/InviteCode';
import type { Member } from './types/Member';
import type { PresenceState } from './types/PresenceState';
import type { PublicConfig } from './types/PublicConfig';
import type { Role } from './types/Role';
import type { User } from './types/User';
import type { VoiceToken } from './types/VoiceToken';
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

const state = (channelId: string | null, extra: Partial<PresenceState> = {}): PresenceState => ({
  channelId,
  app: 'nightly 0.1.0',
  talk: 'vad',
  away: false,
  ...extra,
});

/** Who is where in the busy scenes (concept screen "Im Call"). */
const EVENING: Record<string, PresenceState> = {
  krille: state('raid'),
  ben: state('raid'),
  sasha: state('raid'),
  tim: state('raid'),
  lea: state('raid', { talk: 'ptt' }),
  mo: state('lobby'),
  jule: state('lobby'),
  rico: state('afk', { away: true, app: 'stable 0.0.9' }),
};

const QUIET: Record<string, PresenceState> = {
  krille: state('lobby'),
  rico: state('afk', { away: true, app: 'stable 0.0.9' }),
};

const reject = (code: string, message: string): Promise<never> =>
  Promise.reject<never>({ code, message } satisfies ApiError);

const UNREACHABLE: ApiError = { code: 'unreachable', message: 'Server nicht erreichbar.' };

/** A fixed clock so snapshots and tests do not drift. */
const T0 = Date.UTC(2026, 9, 23, 19, 0, 0);
const at = (minutes: number) => new Date(T0 + minutes * 60_000).toISOString();

export function createMockApi(scene: Scene = DEFAULT_SCENE): Api {
  const users = new Map(GANG.map((u) => [u.id, structuredClone(u)]));
  const channels: Channel[] = structuredClone(CHANNELS) as Channel[];
  const myId = scene === 'member' ? 'ben' : 'krille';
  const presence = new Map<string, PresenceState>(
    Object.entries(structuredClone(scene === 'lobby' ? QUIET : EVENING)),
  );
  if (scene === 'lobby') presence.set(myId, state('lobby'));
  const handlers = new Set<(presence: PresenceMap) => void>();
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
      message: 'Ben ist Raid-Abend beigetreten',
    },
    {
      time: at(-29),
      kind: 'joined',
      userId: 'sasha',
      channelId: 'raid',
      message: 'Sasha ist Raid-Abend beigetreten',
    },
    {
      time: at(-12),
      kind: 'reconnected',
      userId: 'tim',
      channelId: 'raid',
      message: 'Tim hat die Verbindung wiederhergestellt',
    },
    {
      time: at(-3),
      kind: 'invite_redeemed',
      userId: 'lea',
      channelId: null,
      message: 'Lea hat eine Einladung eingelöst',
    },
  ];
  let loggedIn = scene !== 'offline';
  let nextCode = 1;

  const snapshot = (): PresenceMap =>
    Object.fromEntries(
      [...presence].map(([userId, s]) => [
        userId,
        {
          userId,
          displayName: users.get(userId)?.displayName ?? userId,
          state: structuredClone(s),
        },
      ]),
    );
  const notify = () => {
    const map = snapshot();
    handlers.forEach((handler) => handler(map));
  };
  const online = <T>(value: () => T): Promise<T> =>
    scene === 'offline' ? Promise.reject(UNREACHABLE) : Promise.resolve(value());
  const me = () => users.get(myId)!;
  const mustBeAdmin = <T>(value: () => T): Promise<T> =>
    me().role === 'admin' ? online(value) : reject('forbidden', 'Nur für Admins.');
  const token = (room: string): VoiceToken => ({
    wsUrl: MOCK_WS_URL,
    token: `mock.${room}.${myId}`,
    expiresAt: at(60 * 12),
  });
  const sorted = () => [...channels].sort((a, b) => a.position - b.position);

  return {
    me: () =>
      scene === 'offline'
        ? Promise.reject(UNREACHABLE)
        : loggedIn
          ? Promise.resolve(structuredClone(me()))
          : reject('no_session', 'Nicht angemeldet.'),
    redeemInvite(code, displayName) {
      if (scene === 'offline') return Promise.reject(UNREACHABLE);
      const invite = invites.find((i) => i.code === code && i.usedBy === null);
      if (!invite) return reject('invite_invalid', 'Diese Einladung ist ungültig oder verbraucht.');
      const name = displayName.trim();
      if (name.length < 2)
        return reject('name_too_short', 'Der Name braucht mindestens zwei Zeichen.');
      invite.usedBy = myId;
      users.set(myId, { ...me(), displayName: name });
      loggedIn = true;
      events.push({
        time: at(0),
        kind: 'invite_redeemed',
        userId: myId,
        channelId: null,
        message: `${name} hat eine Einladung eingelöst`,
      });
      return Promise.resolve({
        user: structuredClone(me()),
        sessionToken: 'mock-session',
        presence: token('presence'),
      });
    },
    logout() {
      loggedIn = false;
      presence.delete(myId);
      notify();
      return Promise.resolve();
    },
    config: () =>
      online(() => ({
        apiVersion: 1,
        minVersion: scene === 'update' ? '99.0.0' : '0.1.0',
        killedVersions: [],
        updateBaseUrl: 'https://yappa.example/updates',
        livekitWsUrl: MOCK_WS_URL,
      })) as Promise<PublicConfig>,
    channels: () => online(() => structuredClone(sorted())),
    voiceToken: (channelId) =>
      channels.some((c) => c.id === channelId)
        ? online(() => token(channelId))
        : reject('channel_unknown', 'Diesen Channel gibt es nicht.'),
    presence(handler) {
      handlers.add(handler);
      handler(snapshot());
      return Promise.resolve(() => {
        handlers.delete(handler);
      });
    },
    setPresence(patch) {
      if (scene === 'offline') return Promise.reject(UNREACHABLE);
      presence.set(myId, { ...(presence.get(myId) ?? state(null)), ...patch });
      notify();
      return Promise.resolve();
    },
    admin: {
      members: () =>
        mustBeAdmin(() =>
          [...users.values()].map((u): Member => ({
            user: structuredClone(u),
            client: presence.has(u.id)
              ? {
                  appVersion: presence.get(u.id)!.app.split(' ')[1] ?? '0.0.0',
                  protocolVersion: 1,
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
    },
  };
}
