// What every part of the window shares: the engine session, the shell, the API door, and a
// little app-wide state that has no better home until `AppSettings` exist (C1). Handed down
// with a typed context, so components do not take the same three props over and over.
import { createContext } from 'svelte';
import type { Api } from './api';
import type { AppInfo } from './engine';
import { Gang } from './gang.svelte';
import type { Session } from './session.svelte';
import type { Shell } from './shell.svelte';
import type { PublicConfig } from './types/PublicConfig';

/** The sections of the module „Ich“. U3 adds the Einfach/Profi split on top. */
export const ME_SECTIONS = [
  { id: 'konto', label: 'Konto' },
  { id: 'audio', label: 'Audio und Sprechen' },
  { id: 'darstellung', label: 'Darstellung' },
  { id: 'diagnose', label: 'Diagnose' },
  { id: 'hilfe', label: 'Hilfe' },
] as const;

export type MeSection = (typeof ME_SECTIONS)[number]['id'];

export type Theme = 'dark' | 'light';

const THEME_KEY = 'yappa.theme';

export class AppState {
  info = $state<AppInfo | null>(null);
  config = $state<PublicConfig | null>(null);
  meSection = $state<MeSection>('konto');
  theme = $state<Theme>('dark');

  readonly session: Session;
  readonly shell: Shell;
  readonly api: Api;
  readonly gang: Gang;

  constructor(session: Session, shell: Shell, api: Api) {
    this.session = session;
    this.shell = shell;
    this.api = api;
    this.gang = new Gang(api);
    try {
      const stored = localStorage.getItem(THEME_KEY);
      if (stored === 'dark' || stored === 'light') this.theme = stored;
    } catch {
      // No storage: the theme lasts until the window closes.
    }
  }

  setTheme(theme: Theme) {
    this.theme = theme;
    try {
      localStorage.setItem(THEME_KEY, theme);
    } catch {
      // see constructor
    }
  }

  /** Leaves the voice channel: through the control-plane client when logged in (so the
   * presence follows), straight at the engine for a pasted developer invite. */
  leaveVoice() {
    return this.gang.status === 'in' ? this.gang.leave() : this.session.leave();
  }

  /** Opens „Ich“ at a section — from the palette, the dock, or a help link. */
  openMe(section: MeSection) {
    this.meSection = section;
    this.shell.open('me');
  }
}

export const [getApp, setApp] = createContext<AppState>();
