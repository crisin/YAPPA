// Who I am in the gang, which channels there are, who sits where — the control-plane side
// of the UI, next to `Session` (the engine side). Reads only through the Api door.
import type { Api, PresenceEntry } from './api';
import type { ApiError } from './types/ApiError';
import type { Channel } from './types/Channel';
import type { User } from './types/User';

/** `starting` until the stored login is checked; `out` = log in first. */
export type GangStatus = 'starting' | 'out' | 'in';

function message(reason: unknown): string {
  const error = reason as Partial<ApiError> | undefined;
  return error?.message ?? String(reason);
}

export class Gang {
  status = $state<GangStatus>('starting');
  me = $state<User | null>(null);
  channels = $state<Channel[]>([]);
  people = $state<PresenceEntry[]>([]);
  /** The server could not be reached at the last attempt (cache and presence carry on). */
  offline = $state(false);
  /** Last problem with login or joining, for the form or the banner. */
  problem = $state<string | null>(null);
  /** A channel join is on its way. */
  joining = $state<string | null>(null);

  readonly #api: Api;
  #stopPresence: (() => void) | null = null;

  constructor(api: Api) {
    this.#api = api;
  }

  /** Where I sit according to the presence room. */
  get myChannel(): string | null {
    if (!this.me) return null;
    return this.people.find((p) => p.userId === this.me?.id)?.state.channelId ?? null;
  }

  /** Online people in a channel, me first. */
  peopleIn(channelId: string): PresenceEntry[] {
    return this.people.filter((p) => p.state.channelId === channelId);
  }

  channelName(id: string | null): string | null {
    return this.channels.find((c) => c.id === id)?.name ?? null;
  }

  /** At start: the stored login, if any. Never throws — failures become state. */
  async start(): Promise<void> {
    try {
      const me = await this.#api.resume();
      if (me) await this.#enter(me);
      else this.status = 'out';
    } catch (reason) {
      this.offline = (reason as ApiError)?.code === 'unreachable';
      this.problem = message(reason);
      this.status = 'out';
    }
  }

  async login(link: string, displayName: string): Promise<boolean> {
    this.problem = null;
    try {
      const me = await this.#api.redeem(link, displayName);
      await this.#enter(me);
      return true;
    } catch (reason) {
      this.problem = message(reason);
      return false;
    }
  }

  async logout(): Promise<void> {
    this.#stopPresence?.();
    this.#stopPresence = null;
    await this.#api.logout();
    this.offline = false;
    this.me = null;
    this.people = [];
    this.channels = [];
    this.status = 'out';
  }

  async join(channelId: string): Promise<void> {
    if (this.joining || channelId === this.myChannel) return;
    this.problem = null;
    this.joining = channelId;
    try {
      await this.#api.joinChannel(channelId);
    } catch (reason) {
      this.problem = message(reason);
    } finally {
      this.joining = null;
    }
  }

  async leave(): Promise<void> {
    await this.#api.leaveChannel();
  }

  async refreshChannels(): Promise<void> {
    try {
      this.channels = await this.#api.channels();
      this.offline = false;
    } catch (reason) {
      this.offline = (reason as ApiError)?.code === 'unreachable';
    }
  }

  stop() {
    this.#stopPresence?.();
    this.#stopPresence = null;
  }

  async #enter(me: User) {
    this.me = me;
    this.status = 'in';
    this.offline = false;
    await this.refreshChannels();
    this.#stopPresence?.();
    this.#stopPresence = await this.#api.presence((people) => (this.people = people));
  }
}
