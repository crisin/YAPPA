// The UI's only door to the engine. Speaks engine-protocol (generated types), nothing else.
// Outside Tauri (plain `npm run dev` in a browser, and in tests) a mock answers, so the UI
// stays workable without audio hardware or a server.
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Command } from './types/Command';
import type { Event } from './types/Event';
import type { Invite } from './types/Invite';
import type { LogEntry } from './types/LogEntry';
import type { Settings } from './types/Settings';
import { createMockEngine } from './mockEngine';
import { sceneFromUrl } from './scene';

export interface AppInfo {
  version: string;
  /** 0 = browser mode, no real engine. */
  protocolVersion: number;
  logDir: string;
  /** Open the debug panel at start (development aid). */
  debugPanel: boolean;
}

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
};

export const engine: Engine = isTauri() ? tauriEngine : createMockEngine(sceneFromUrl());
