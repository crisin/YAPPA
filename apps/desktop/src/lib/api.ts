// The UI's second door (ADR-004): everything that is not audio — who I am, channels, who is
// where, admin. It speaks api-types (generated) and nothing else. In the app the Tauri
// commands of crates/client answer (U5); in the browser and in tests the mock gang does.
// Session and voice tokens never come through this door: joining a channel is one call.
import type { Channel } from './types/Channel';
import type { EventEntry } from './types/EventEntry';
import type { InviteCode } from './types/InviteCode';
import type { Member } from './types/Member';
import type { PresenceEntry } from './types/PresenceEntry';
import type { PublicConfig } from './types/PublicConfig';
import type { Role } from './types/Role';
import type { TalkMode } from './types/TalkMode';
import type { User } from './types/User';
import { engine } from './engine';
import { createMockApi } from './mockApi';
import { sceneFromUrl } from './scene';

export type { PresenceEntry };

/** What the UI may change about its own presence; the channel follows join/leave. */
export interface PresencePatch {
  talk?: TalkMode;
  away?: boolean;
}

/** Rejections carry an `ApiError` shape: a stable code and a message for the user. */
export interface Api {
  /**
   * The login stored on this machine, checked with the server; joins the presence room.
   * `null` = nobody logged in here yet. Rejects with `unreachable` when the server is away.
   */
  resume(): Promise<User | null>;
  /** First login: an invite link (`yappa://invite/<code>?server=…`) and a display name. */
  redeem(link: string, displayName: string): Promise<User>;
  logout(): Promise<void>;
  config(): Promise<PublicConfig>;
  /** Sorted by position; the cached list when the server is away. */
  channels(): Promise<Channel[]>;
  /**
   * Everybody online — me first, then by name — once right away and on every change. The
   * returned function stops the updates.
   */
  presence(handler: (people: PresenceEntry[]) => void): Promise<() => void>;
  setPresence(patch: PresencePatch): Promise<void>;
  /** Into a channel: token, engine join and presence in one step. */
  joinChannel(channelId: string): Promise<void>;
  leaveChannel(): Promise<void>;
  admin: {
    members(): Promise<Member[]>;
    setRole(userId: string, role: Role): Promise<void>;
    kick(userId: string): Promise<void>;
    invites(): Promise<InviteCode[]>;
    createInvite(): Promise<InviteCode>;
    revokeInvite(code: string): Promise<void>;
    createChannel(name: string): Promise<Channel>;
    renameChannel(id: string, name: string): Promise<void>;
    deleteChannel(id: string): Promise<void>;
    /** Puts the channel at `position` and shifts the others. */
    moveChannel(id: string, position: number): Promise<void>;
    events(): Promise<EventEntry[]>;
  };
}

/** `yappa://invite/…` (control plane) or `yappa1.…` (a developer invite straight to a room). */
export function inviteKind(text: string): 'link' | 'direct' | null {
  const compact = text.replace(/\s+/g, '');
  if (compact.startsWith('yappa://invite/')) return 'link';
  if (compact.startsWith('yappa1.')) return 'direct';
  return null;
}

export const api: Api = createMockApi(sceneFromUrl(), engine);
