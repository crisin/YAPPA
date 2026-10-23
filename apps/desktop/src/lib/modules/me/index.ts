// The module „Ich“ (ADR-004): settings, diagnosis and help in the shell instead of an
// overlay, so the voice dock stays reachable during a call.
import { CircleUser } from '@lucide/svelte';
import { ME_SECTIONS, type AppState } from '../../app.svelte';
import type { CommandDef, ModuleDef } from '../../shell.svelte';
import MeSidebar from './MeSidebar.svelte';
import MeStage from './MeStage.svelte';

export function meModule(app: AppState): ModuleDef {
  return {
    id: 'me',
    label: 'Ich',
    icon: CircleUser,
    slot: 'bottom',
    sidebar: MeSidebar,
    stage: MeStage,
    crumb: () => ME_SECTIONS.find((s) => s.id === app.meSection)?.label ?? null,
    commands: () => {
      const sections: CommandDef[] = ME_SECTIONS.map((section) => ({
        id: `me:${section.id}`,
        label: section.label,
        group: 'Einstellungen',
        run: () => app.openMe(section.id),
      }));
      return [
        ...sections,
        {
          id: 'me:theme',
          label: app.theme === 'dark' ? 'Helles Farbschema' : 'Dunkles Farbschema',
          group: 'Einstellungen',
          keywords: ['theme', 'farbe', 'dark', 'light'],
          run: () => app.setTheme(app.theme === 'dark' ? 'light' : 'dark'),
        },
        {
          id: 'me:view',
          label: app.view === 'pro' ? 'Einstellungen: Einfach' : 'Einstellungen: Profi',
          group: 'Einstellungen',
          keywords: ['profi', 'einfach', 'experte', 'ansicht'],
          run: () => app.setView(app.view === 'pro' ? 'simple' : 'pro'),
        },
      ];
    },
  };
}
