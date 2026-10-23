// The UI door in the app (U5): the Tauri commands of crates/client (src-tauri/src/api.rs).
// Session and voice tokens stay in Rust — joining a channel is `voice_join`.
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Api, PresenceEntry } from './api';
import type { ApiError } from './types/ApiError';
import type { Channel } from './types/Channel';
import type { PublicConfig } from './types/PublicConfig';
import type { User } from './types/User';

/** The admin endpoints exist in the control plane and crates/client; the UI gets them in U6. */
const notYet = (): Promise<never> =>
  Promise.reject<never>({
    code: 'not_available',
    message: 'Die Verwaltung kommt mit dem Admin-Bereich.',
  } satisfies ApiError);

export const tauriApi: Api = {
  resume: () => invoke<User | null>('api_resume'),
  redeem: (link, displayName) => invoke<User>('api_redeem', { link, displayName }),
  logout: () => invoke<void>('api_logout'),
  config: () => invoke<PublicConfig>('api_config'),
  channels: () => invoke<Channel[]>('api_channels'),
  async presence(handler) {
    // Listen first, then read: a change between the two is not lost.
    const stop = await listen<PresenceEntry[]>('presence', (event) => handler(event.payload));
    handler(await invoke<PresenceEntry[]>('api_presence'));
    return stop;
  },
  setPresence: (patch) => invoke<void>('api_set_presence', { patch }),
  joinChannel: (channelId) => invoke<void>('voice_join', { channelId }),
  leaveChannel: () => invoke<void>('voice_leave'),
  async inviteLinks(handler) {
    const stop = await listen<string>('invite-link', (event) => handler(event.payload));
    const first = await invoke<string | null>('pending_invite_link');
    if (first) handler(first);
    return stop;
  },
  admin: {
    members: notYet,
    setRole: notYet,
    kick: notYet,
    invites: notYet,
    createInvite: notYet,
    revokeInvite: notYet,
    createChannel: notYet,
    renameChannel: notYet,
    deleteChannel: notYet,
    moveChannel: notYet,
    events: notYet,
  },
};
