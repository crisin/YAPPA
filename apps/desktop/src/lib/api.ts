// The UI's second door (ADR-004): everything that is not audio — who I am, channels, who is
// where, admin. It speaks api-types (generated) and nothing else. Until the client crate
// exists (C1/U5) the door is the mock in every environment, so the UI can be built and the
// browser mode shows "Die Gang" without a server.
import type { Channel } from './types/Channel';
import type { EventEntry } from './types/EventEntry';
import type { InviteCode } from './types/InviteCode';
import type { Member } from './types/Member';
import type { PresenceEntry } from './types/PresenceEntry';
import type { PresenceState } from './types/PresenceState';
import type { PublicConfig } from './types/PublicConfig';
import type { Role } from './types/Role';
import type { Session } from './types/Session';
import type { User } from './types/User';
import type { VoiceToken } from './types/VoiceToken';
import { createMockApi } from './mockApi';
import { sceneFromUrl } from './scene';

export type { PresenceEntry };

/** Everybody who is online, by user id. Not in the map = app not running. */
export type PresenceMap = Record<string, PresenceEntry>;

/** Rejections carry an `ApiError` shape: a stable code and a message for the user. */
export interface Api {
  /** Who I am; rejects with `no_session` when nobody is logged in here. */
  me(): Promise<User>;
  redeemInvite(code: string, displayName: string): Promise<Session>;
  logout(): Promise<void>;
  config(): Promise<PublicConfig>;
  /** Sorted by position. */
  channels(): Promise<Channel[]>;
  voiceToken(channelId: string): Promise<VoiceToken>;
  /**
   * The whole presence map on every change, once right away. The returned function stops
   * the updates.
   */
  presence(handler: (presence: PresenceMap) => void): Promise<() => void>;
  /** Changes my own attributes in the presence room. */
  setPresence(patch: Partial<PresenceState>): Promise<void>;
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

export const api: Api = createMockApi(sceneFromUrl());
