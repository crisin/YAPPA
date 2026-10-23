// What the admin module shows and changes (U6): members with their client and presence,
// invites, channels, the event log, the version policy. Everything goes through the API door;
// the control plane checks the role on every call, this only keeps the lists and the last
// problem. Channel changes refresh the gang's list, so the channel tree follows at once.
import type { Api } from './api';
import type { Gang } from './gang.svelte';
import type { ApiError } from './types/ApiError';
import type { EventEntry } from './types/EventEntry';
import type { InviteCode } from './types/InviteCode';
import type { Member } from './types/Member';
import type { PublicConfig } from './types/PublicConfig';
import type { Role } from './types/Role';

export const ADMIN_SECTIONS = [
  { id: 'leute', label: 'Leute' },
  { id: 'einladungen', label: 'Einladungen' },
  { id: 'channels', label: 'Channels' },
  { id: 'betrieb', label: 'Betrieb' },
] as const;

export type AdminSection = (typeof ADMIN_SECTIONS)[number]['id'];

const message = (reason: unknown) =>
  (reason as ApiError)?.message ?? 'Das hat nicht geklappt. Bitte später noch einmal versuchen.';

export class Admin {
  section = $state<AdminSection>('leute');
  members = $state<Member[]>([]);
  invites = $state<InviteCode[]>([]);
  events = $state<EventEntry[]>([]);
  /** The link of the invite just created, to copy and send. */
  newLink = $state<string | null>(null);
  problem = $state<string | null>(null);
  busy = $state(false);

  readonly #api: Api;
  readonly #gang: Gang;
  readonly #onConfig: (config: PublicConfig) => void;

  constructor(api: Api, gang: Gang, onConfig: (config: PublicConfig) => void) {
    this.#api = api;
    this.#gang = gang;
    this.#onConfig = onConfig;
  }

  /** Runs one call; a failure becomes `problem` instead of an exception. */
  async #do<T>(work: () => Promise<T>): Promise<T | undefined> {
    this.problem = null;
    this.busy = true;
    try {
      return await work();
    } catch (reason) {
      this.problem = message(reason);
      return undefined;
    } finally {
      this.busy = false;
    }
  }

  /** Loads what the open section shows. */
  refresh() {
    switch (this.section) {
      case 'leute':
        return this.#do(async () => void (this.members = await this.#api.admin.members()));
      case 'einladungen':
        return this.#do(async () => void (this.invites = await this.#api.admin.invites()));
      case 'channels':
        return this.#do(() => this.#gang.refreshChannels());
      case 'betrieb':
        return this.#do(async () => void (this.events = await this.#api.admin.events()));
    }
  }

  open(section: AdminSection) {
    this.section = section;
    return this.refresh();
  }

  setRole(userId: string, role: Role) {
    return this.#do(async () => {
      await this.#api.admin.setRole(userId, role);
      this.members = await this.#api.admin.members();
    });
  }

  kick(userId: string) {
    return this.#do(() => this.#api.admin.kick(userId));
  }

  createInvite() {
    return this.#do(async () => {
      const invite = await this.#api.admin.createInvite();
      this.newLink = await this.#api.admin.inviteLink(invite.code);
      this.invites = await this.#api.admin.invites();
    });
  }

  revokeInvite(code: string) {
    return this.#do(async () => {
      await this.#api.admin.revokeInvite(code);
      this.invites = await this.#api.admin.invites();
    });
  }

  addChannel(name: string) {
    return this.#do(async () => {
      await this.#api.admin.createChannel(name);
      await this.#gang.refreshChannels();
    });
  }

  renameChannel(id: string, name: string) {
    return this.#do(async () => {
      await this.#api.admin.renameChannel(id, name);
      await this.#gang.refreshChannels();
    });
  }

  moveChannel(id: string, position: number) {
    return this.#do(async () => {
      await this.#api.admin.moveChannel(id, position);
      await this.#gang.refreshChannels();
    });
  }

  deleteChannel(id: string) {
    return this.#do(async () => {
      await this.#api.admin.deleteChannel(id);
      await this.#gang.refreshChannels();
    });
  }

  setPolicy(minVersion: string, killedVersions: string[]) {
    return this.#do(async () => {
      this.#onConfig(await this.#api.admin.setPolicy({ minVersion, killedVersions }));
    });
  }
}
