// The UI's only door to the engine. Speaks engine-protocol (generated types), nothing else.
// Outside Tauri (plain `npm run dev` in a browser, and in tests) a mock answers, so the UI
// stays workable without audio hardware or a server.
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { AppInfo } from './types/AppInfo';
import type { Command } from './types/Command';
import type { Event } from './types/Event';
import type { Invite } from './types/Invite';
import type { LogEntry } from './types/LogEntry';
import type { Settings } from './types/Settings';
import type { Sound } from './types/Sound';
import { createMockEngine } from './mockEngine';
import { sceneFromUrl } from './scene';

export type { AppInfo };

export interface Engine {
  info(): Promise<AppInfo>;
  settings(): Promise<Settings>;
  send(command: Command): Promise<void>;
  /** Every event from the engine, until the returned function is called. */
  subscribe(handler: (event: Event) => void): Promise<() => void>;
  /** What was logged before the UI listened; also makes the engine resend its state. */
  recentLogs(): Promise<LogEntry[]>;
  /** Rejects with a reason when the text is not a usable invite. */
  decodeInvite(text: string): Promise<Invite>;
  /** Writes the diagnostics file and resolves with its path. */
  exportLogs(): Promise<string>;
  /** Opens a web link (http/https only) in the system browser. */
  openUrl(url: string): Promise<void>;
  /** The soundboard library (ADR-010): every sound file in the app's sound folder. */
  sounds(): Promise<Sound[]>;
  /** Copies a picked file into the sound folder; rejects with a German reason. */
  addSound(name: string, bytes: Uint8Array): Promise<Sound>;
  /** Shows the sound folder in the file manager — drop files there by hand. */
  openSoundFolder(): Promise<void>;
}

/** Bytes as base64, the way `soundboard_add` wants them (chunked: a spread of a million
 * arguments would overflow the call stack). */
export function toBase64(bytes: Uint8Array): string {
  let binary = '';
  for (let i = 0; i < bytes.length; i += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(binary);
}

const tauriEngine: Engine = {
  info: () => invoke<AppInfo>('app_info'),
  settings: () => invoke<Settings>('get_settings'),
  send: (command) => invoke<void>('engine_command', { command }),
  subscribe: (handler) => listen<Event>('engine-event', (message) => handler(message.payload)),
  recentLogs: () => invoke<LogEntry[]>('recent_logs'),
  decodeInvite: (text) => invoke<Invite>('decode_invite', { text }),
  exportLogs: () => invoke<string>('export_logs'),
  openUrl: (url) => invoke<void>('open_url', { url }),
  sounds: () => invoke<Sound[]>('soundboard_list'),
  addSound: (name, bytes) => invoke<Sound>('soundboard_add', { name, base64: toBase64(bytes) }),
  openSoundFolder: () => invoke<void>('soundboard_open_folder'),
};

export const engine: Engine = isTauri() ? tauriEngine : createMockEngine(sceneFromUrl());
