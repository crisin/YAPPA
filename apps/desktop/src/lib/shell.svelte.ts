// The shell's own state (ADR-004): which module is open, the inspector, the banner slot and
// the command palette. Modules are entries in a registry; the shell renders the registry
// and never names a module itself. Nothing in here knows the engine or the API.
import type { Component } from 'svelte';
import type { Role } from './types/Role';

/** One thing the palette can do: open a channel, a person, a setting, run an action. */
export interface CommandDef {
  id: string;
  label: string;
  /** Heading the palette groups it under, e.g. „Channels“, „Leute“, „Einstellungen“. */
  group: string;
  keywords?: string[];
  run(): void;
}

/** A module: a rail slot, its own sidebar and stage, its commands. Icons are components. */
export interface ModuleDef {
  id: string;
  label: string;
  icon: Component;
  /** Shown only to this role and above (`admin` sees everything). */
  role?: Role;
  /** Where the rail shows it: the module list at the top, or next to „Ich“ at the bottom. */
  slot?: 'main' | 'bottom';
  sidebar: Component;
  stage: Component;
  /** What the inspector shows for this module; no inspector without it. */
  inspector?: Component;
  /** Breadcrumb after the module label, e.g. the room or the settings section. */
  crumb?: () => string | null;
  commands?: () => CommandDef[];
}

/** Banner priorities from the vision (section 5): lower number wins the one slot. */
export const BANNER = {
  blocked: 1,
  error: 2,
  reconnecting: 3,
  offline: 4,
  update: 5,
} as const;

export interface Banner {
  id: string;
  priority: number;
  text: string;
  action?: { label: string; run(): void };
  /** Can the person close it? Blocking banners cannot. */
  dismissible: boolean;
  /** Runs when the person closes it, e.g. to forget the error that caused it. */
  onDismiss?(): void;
}

export interface InspectorContent {
  /** What kind of thing is shown: `person`, `channel`, `soundboard`, … */
  kind: string;
  id: string;
}

const ROLE_RANK: Record<Role, number> = { member: 0, admin: 1 };

export class Shell {
  modules = $state<ModuleDef[]>([]);
  role = $state<Role>('member');
  active = $state<string>('');
  inspector = $state<{ open: boolean; content: InspectorContent | null }>({
    open: false,
    content: null,
  });
  banners = $state<Banner[]>([]);
  palette = $state({ open: false, query: '' });

  /** Modules visited before the active one, newest last — for `Esc`/back. */
  #history: string[] = [];
  /** Whether the inspector was open the last time each module was active. */
  #inspectorWasOpen = new Map<string, boolean>();

  /** Modules this role may see, in registry order. */
  get visible(): ModuleDef[] {
    return this.modules.filter((m) => ROLE_RANK[m.role ?? 'member'] <= ROLE_RANK[this.role]);
  }

  /** The rail's two groups. */
  get railMain(): ModuleDef[] {
    return this.visible.filter((m) => (m.slot ?? 'main') === 'main');
  }

  get railBottom(): ModuleDef[] {
    return this.visible.filter((m) => m.slot === 'bottom');
  }

  get activeModule(): ModuleDef | undefined {
    return this.visible.find((m) => m.id === this.active);
  }

  /** The one banner shown: highest priority first, then the newest. */
  get banner(): Banner | undefined {
    return this.banners[0];
  }

  register(module: ModuleDef) {
    const index = this.modules.findIndex((m) => m.id === module.id);
    if (index >= 0) this.modules[index] = module;
    else this.modules.push(module);
    if (this.active === '' && this.visibleId(module.id)) this.active = module.id;
  }

  /** Opens a module; unknown or forbidden ids do nothing. */
  open(id: string) {
    if (id === this.active || !this.visibleId(id)) return;
    if (this.active !== '') {
      this.#inspectorWasOpen.set(this.active, this.inspector.open);
      this.#history = [...this.#history.filter((h) => h !== this.active), this.active].slice(-8);
    }
    this.active = id;
    this.inspector = { open: this.#inspectorWasOpen.get(id) ?? false, content: null };
  }

  /** `Esc`: back to the module before this one, or the first visible one. */
  back() {
    const previous = this.#history.pop();
    const target = previous && this.visibleId(previous) ? previous : this.visible[0]?.id;
    if (target && target !== this.active) {
      const history = this.#history;
      this.open(target);
      this.#history = history;
    }
  }

  openInspector(content: InspectorContent) {
    this.inspector = { open: true, content };
  }

  closeInspector() {
    this.inspector = { open: false, content: null };
  }

  /** Shows a banner; the same id replaces its older self. */
  showBanner(banner: Banner) {
    const others = this.banners.filter((b) => b.id !== banner.id);
    this.banners = [...others, banner].sort((a, b) => a.priority - b.priority);
  }

  dismissBanner(id: string) {
    const banner = this.banners.find((b) => b.id === id);
    if (!banner?.dismissible) return;
    this.banners = this.banners.filter((b) => b.id !== id);
    banner.onDismiss?.();
  }

  /** Clears a banner whatever its dismissibility — for the thing that caused it going away. */
  clearBanner(id: string) {
    this.banners = this.banners.filter((b) => b.id !== id);
  }

  /** Everything the palette offers: switching modules, then each visible module's own. */
  commands(): CommandDef[] {
    const switching: CommandDef[] = this.visible.map((m) => ({
      id: `module:${m.id}`,
      label: m.label,
      group: 'Bereiche',
      run: () => this.open(m.id),
    }));
    return [...switching, ...this.visible.flatMap((m) => m.commands?.() ?? [])];
  }

  openPalette() {
    this.palette = { open: true, query: '' };
  }

  closePalette() {
    this.palette = { open: false, query: '' };
  }

  togglePalette() {
    if (this.palette.open) this.closePalette();
    else this.openPalette();
  }

  /** When the role changes (login, role edit) a module may vanish from under us. */
  setRole(role: Role) {
    this.role = role;
    if (!this.visibleId(this.active)) this.active = this.visible[0]?.id ?? '';
  }

  private visibleId(id: string): boolean {
    return this.visible.some((m) => m.id === id);
  }
}
