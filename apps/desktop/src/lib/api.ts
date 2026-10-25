// The UI's second door (ADR-004): everything that is not audio — who I am, channels, who is
// where, admin. It speaks api-types (generated) and nothing else. In the app the Tauri
// commands of crates/client answer (`tauriApi.ts`); in the browser and in tests the mock
// gang does.
// Session and voice tokens never come through this door: joining a channel is one call.
import type { AppSettings } from './types/AppSettings';
import type { Channel } from './types/Channel';
import type { EventEntry } from './types/EventEntry';
import type { InviteCode } from './types/InviteCode';
import type { KeyAction } from './types/KeyAction';
import type { Member } from './types/Member';
import type { PresenceEntry } from './types/PresenceEntry';
import type { PublicConfig } from './types/PublicConfig';
import type { Role } from './types/Role';
import type { TalkMode } from './types/TalkMode';
import type { UpdateStatus } from './types/UpdateStatus';
import type { User } from './types/User';
import type { VersionPolicy } from './types/VersionPolicy';
import { isTauri } from '@tauri-apps/api/core';
import { engine } from './engine';
import { createMockApi } from './mockApi';
import { sceneFromUrl } from './scene';
import { tauriApi } from './tauriApi';

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
  /**
   * Invite links the OS hands over (a clicked `yappa://invite/…`): the one that started the
   * app right away, later ones as they come. The returned function stops listening.
   */
  inviteLinks(handler: (link: string) => void): Promise<() => void>;
  /** The app's own settings (view, theme, PTT key, …), stored per profile. */
  appSettings(): Promise<AppSettings>;
  setAppSettings(settings: AppSettings): Promise<void>;
  /**
   * Waits for the next key or mouse button and makes it the key of `action` (push-to-talk,
   * mute, deafen); `null` when cancelled (Escape) or nothing came within 10 s. Rejects with
   * `key_taken` when another action already has that key.
   */
  captureKey(action: KeyAction): Promise<string | null>;
  /** False when the global hook is not running: the key then only works in the window. */
  hotkeysGlobal(): Promise<boolean>;
  /**
   * Looks at `<update_base_url>/<channel>/latest.json` (D1); a newer version is downloaded
   * and verified before this answers `ready`. Never rejects for a failed check — that is a
   * status.
   */
  checkUpdate(): Promise<UpdateStatus>;
  /** Installs the downloaded update and restarts the app. Rejects with `no_update`. */
  installUpdate(): Promise<void>;
  admin: {
    members(): Promise<Member[]>;
    setRole(userId: string, role: Role): Promise<void>;
    kick(userId: string): Promise<void>;
    invites(): Promise<InviteCode[]>;
    /** A new invite, valid 7 days, for one person. */
    createInvite(): Promise<InviteCode>;
    /** The clickable `yappa://invite/<code>?server=…` link for a code. */
    inviteLink(code: string): Promise<string>;
    revokeInvite(code: string): Promise<void>;
    createChannel(name: string): Promise<Channel>;
    renameChannel(id: string, name: string): Promise<void>;
    deleteChannel(id: string): Promise<void>;
    /** Puts the channel at `position` and shifts the others. */
    moveChannel(id: string, position: number): Promise<void>;
    /** Newest first. */
    events(): Promise<EventEntry[]>;
    /** Minimum version and kill list; answers the config that now applies. */
    setPolicy(policy: VersionPolicy): Promise<PublicConfig>;
  };
}

/** `yappa://invite/…` (control plane) or `yappa1.…` (a developer invite straight to a room). */
export function inviteKind(text: string): 'link' | 'direct' | null {
  const compact = text.replace(/\s+/g, '');
  if (compact.startsWith('yappa://invite/')) return 'link';
  if (compact.startsWith('yappa1.')) return 'direct';
  return null;
}

export const api: Api = isTauri() ? tauriApi : createMockApi(sceneFromUrl(), engine);
