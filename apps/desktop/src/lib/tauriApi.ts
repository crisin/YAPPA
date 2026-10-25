// The UI door in the app (U5): the Tauri commands of crates/client (src-tauri/src/api.rs).
// Session and voice tokens stay in Rust — joining a channel is `voice_join`.
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Api, PresenceEntry } from './api';
import type { ApiError } from './types/ApiError';
import type { AppSettings } from './types/AppSettings';
import type { Channel } from './types/Channel';
import type { EventEntry } from './types/EventEntry';
import type { InviteCode } from './types/InviteCode';
import type { Member } from './types/Member';
import type { PublicConfig } from './types/PublicConfig';
import type { UpdateStatus } from './types/UpdateStatus';
import type { User } from './types/User';

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
  appSettings: () => invoke<AppSettings>('app_settings'),
  setAppSettings: (settings) => invoke<void>('set_app_settings', { settings }),
  captureKey: (action) => invoke<string | null>('capture_key', { action }),
  hotkeysGlobal: () => invoke<boolean>('hotkeys_global'),
  checkUpdate: () => invoke<UpdateStatus>('check_update'),
  installUpdate: () => invoke<void>('install_update'),
  admin: {
    members: () => invoke<Member[]>('admin_members'),
    setRole: (userId, role) => invoke<void>('admin_set_role', { userId, role }),
    kick: (userId) => invoke<void>('admin_kick', { userId }),
    invites: () => invoke<InviteCode[]>('admin_invites'),
    createInvite: () => invoke<InviteCode>('admin_create_invite'),
    inviteLink: (code) => invoke<string>('admin_invite_link', { code }),
    revokeInvite: (code) => invoke<void>('admin_revoke_invite', { code }),
    createChannel: (name) => invoke<Channel>('admin_create_channel', { name }),
    renameChannel: (id, name) => invoke<void>('admin_rename_channel', { id, name }),
    deleteChannel: (id) => invoke<void>('admin_delete_channel', { id }),
    moveChannel: (id, position) => invoke<void>('admin_move_channel', { id, position }),
    events: () => invoke<EventEntry[]>('admin_events'),
    setPolicy: (policy) => invoke<PublicConfig>('admin_set_policy', { policy }),
  },
};
