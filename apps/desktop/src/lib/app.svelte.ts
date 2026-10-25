// What every part of the window shares: the engine session, the shell, the API door, and
// the app settings (view, theme, …). Handed down with a typed context, so components do not
// take the same three props over and over.
import { createContext } from 'svelte';
import { Admin } from './admin.svelte';
import type { Api } from './api';
import type { AppInfo } from './engine';
import { Gang } from './gang.svelte';
import type { Session } from './session.svelte';
import type { Shell } from './shell.svelte';
import type { AppSettings } from './types/AppSettings';
import type { PublicConfig } from './types/PublicConfig';
import type { SettingsView } from './types/SettingsView';
import type { TransmitMode } from './types/TransmitMode';
import { Updater } from './updater.svelte';

/** The sections of the module „Ich“. `pro` ones show only in the Profi view; both views write
 * the same settings (concept: Einfach/Profi is a property of the settings). */
export const ME_SECTIONS = [
  { id: 'audio', label: 'Audio und Sprechen', pro: false },
  { id: 'signalkette', label: 'Signalkette', pro: true },
  { id: 'tasten', label: 'Tasten', pro: true },
  { id: 'netzwerk', label: 'Netzwerk', pro: true },
  { id: 'darstellung', label: 'Darstellung', pro: false },
  { id: 'benachrichtigungen', label: 'Benachrichtigungen', pro: false },
  { id: 'konto', label: 'Konto', pro: false },
  { id: 'updates', label: 'Updates', pro: true },
  { id: 'diagnose', label: 'Diagnose', pro: true },
  { id: 'hilfe', label: 'Hilfe', pro: false },
] as const;

export type MeSection = (typeof ME_SECTIONS)[number]['id'];

export type Theme = 'dark' | 'light';

/** The steps after the invite was redeemed (step 1 is the login card itself). */
export type OnboardingStep = 'mic' | 'talk';

/** The theme is also kept in the browser storage, so the first frame already has the right
 * colours — the app settings arrive a moment later over IPC. */
const THEME_KEY = 'yappa.theme';

export class AppState {
  info = $state<AppInfo | null>(null);
  config = $state<PublicConfig | null>(null);
  meSection = $state<MeSection>('audio');
  theme = $state<Theme>('dark');
  appSettings = $state<AppSettings | null>(null);
  /** Set right after a first login: the onboarding runs instead of the shell until done. */
  onboarding = $state<OnboardingStep | null>(null);
  /**
   * „Mich selbst hören“ as last switched on the Signalkette page (A5), for this run of the
   * app; `null` = not touched yet, then the page decides from what is on the ears.
   */
  hearMyself = $state<boolean | null>(null);
  view = $derived<SettingsView>(this.appSettings?.view ?? 'simple');

  readonly session: Session;
  readonly shell: Shell;
  readonly api: Api;
  readonly gang: Gang;
  readonly admin: Admin;
  readonly updater: Updater;

  constructor(session: Session, shell: Shell, api: Api) {
    this.session = session;
    this.shell = shell;
    this.api = api;
    this.gang = new Gang(api);
    this.updater = new Updater(api);
    this.admin = new Admin(api, this.gang, (config) => (this.config = config));
    try {
      const stored = localStorage.getItem(THEME_KEY);
      if (stored === 'dark' || stored === 'light') this.theme = stored;
    } catch {
      // No storage: the theme lasts until the window closes.
    }
  }

  /** The onboarding is the whole window: nobody logged in (and no developer invite on its
   * way into a room), or the steps after the first login. */
  get showOnboarding(): boolean {
    return (
      this.onboarding !== null ||
      (this.gang.status === 'out' &&
        !this.session.inRoom &&
        this.session.connection === 'disconnected')
    );
  }

  /** Reads the app settings once at start; their theme wins over the browser storage. */
  async loadAppSettings() {
    const settings = await this.api.appSettings();
    this.appSettings = settings;
    if (settings.theme === 'dark' || settings.theme === 'light')
      this.#rememberTheme(settings.theme);
  }

  /** Changes some app settings. Reads the file first: the shell writes into it too (the PTT
   * key), and a stale copy here must not undo that. */
  async updateAppSettings(patch: Partial<AppSettings>) {
    const next = { ...(await this.api.appSettings()), ...patch };
    this.appSettings = next;
    await this.api.setAppSettings(next);
  }

  setTheme(theme: Theme) {
    this.#rememberTheme(theme);
    return this.updateAppSettings({ theme });
  }

  #rememberTheme(theme: Theme) {
    this.theme = theme;
    try {
      localStorage.setItem(THEME_KEY, theme);
    } catch {
      // see constructor
    }
  }

  /** Einfach or Profi. Leaving Profi on a pro-only section goes back to „Audio und Sprechen“. */
  setView(view: SettingsView) {
    if (view === 'simple' && ME_SECTIONS.find((s) => s.id === this.meSection)?.pro) {
      this.meSection = 'audio';
    }
    return this.updateAppSettings({ view });
  }

  /** Leaves the voice channel: through the control-plane client when logged in (so the
   * presence follows), straight at the engine for a pasted developer invite. */
  leaveVoice() {
    return this.gang.status === 'in' ? this.gang.leave() : this.session.leave();
  }

  /** Installs the downloaded update (D1): never in the middle of a call — the call is left
   * first — then the app restarts. */
  async installUpdate() {
    if (this.session.inRoom) await this.leaveVoice();
    await this.updater.install();
  }

  /** The speaking mode, in the engine and as presence (others see „PTT“ on my tile). */
  async setTransmitMode(mode: TransmitMode) {
    await this.session.setTransmitMode(mode);
    const talk = { voiceActivation: 'vad', pushToTalk: 'ptt', alwaysOn: 'open' } as const;
    if (this.gang.status === 'in') await this.api.setPresence({ talk: talk[mode] }).catch(() => {});
  }

  /** Ends the onboarding in the first channel — the lobby. */
  async finishOnboarding() {
    this.onboarding = null;
    const lobby = this.gang.channels[0];
    if (lobby) await this.gang.join(lobby.id);
  }

  /** Opens „Ich“ at a section — from the palette, the dock, or a help link. A pro section
   * switches to the Profi view, so the navigation shows where one is. */
  openMe(section: MeSection) {
    if (ME_SECTIONS.find((s) => s.id === section)?.pro && this.view !== 'pro') {
      void this.setView('pro');
    }
    this.meSection = section;
    this.shell.open('me');
  }
}

export const [getApp, setApp] = createContext<AppState>();
