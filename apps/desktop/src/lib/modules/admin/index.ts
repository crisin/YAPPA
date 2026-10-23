// The admin module (U6, ADR-004): people, invites, channels, operations. Only admins see it
// in the rail; the control plane checks the role on every call anyway.
import { Shield } from '@lucide/svelte';
import { ADMIN_SECTIONS } from '../../admin.svelte';
import type { AppState } from '../../app.svelte';
import type { CommandDef, ModuleDef } from '../../shell.svelte';
import AdminSidebar from './AdminSidebar.svelte';
import AdminStage from './AdminStage.svelte';

export function adminModule(app: AppState): ModuleDef {
  const { admin, shell } = app;
  const open = (section: (typeof ADMIN_SECTIONS)[number]['id']) => {
    shell.open('admin');
    void admin.open(section);
  };
  return {
    id: 'admin',
    label: 'Admin',
    icon: Shield,
    role: 'admin',
    slot: 'bottom',
    sidebar: AdminSidebar,
    stage: AdminStage,
    crumb: () => ADMIN_SECTIONS.find((s) => s.id === admin.section)?.label ?? null,
    commands: (): CommandDef[] => [
      ...ADMIN_SECTIONS.map((section) => ({
        id: `admin:${section.id}`,
        label: `Admin: ${section.label}`,
        group: 'Admin',
        run: () => open(section.id),
      })),
      {
        id: 'admin:invite',
        label: 'Einladung erstellen',
        group: 'Admin',
        keywords: ['einladen', 'invite', 'link'],
        run: () => {
          open('einladungen');
          void admin.createInvite();
        },
      },
    ],
  };
}
