// The voice module (ADR-004): rail slot, sidebar, stage, person inspector, palette entries.
import { AudioLines } from '@lucide/svelte';
import type { AppState } from '../../app.svelte';
import type { CommandDef, ModuleDef } from '../../shell.svelte';
import PersonInspector from './PersonInspector.svelte';
import VoiceSidebar from './VoiceSidebar.svelte';
import VoiceStage from './VoiceStage.svelte';

export function voiceModule(app: AppState): ModuleDef {
  const { session, shell, gang } = app;
  return {
    id: 'voice',
    label: 'Voice',
    icon: AudioLines,
    sidebar: VoiceSidebar,
    stage: VoiceStage,
    inspector: PersonInspector,
    crumb: () => (session.inRoom ? (gang.channelName(session.room) ?? session.room) : null),
    commands: () => {
      const actions: CommandDef[] = [
        {
          id: 'voice:mute',
          label: session.muted ? 'Mikrofon wieder an' : 'Mikrofon stummschalten',
          group: 'Aktionen',
          keywords: ['mute', 'stumm'],
          run: () => void session.toggleMute(),
        },
        {
          id: 'voice:deafen',
          label: session.deafened ? 'Ton wieder an' : 'Ton stummschalten',
          group: 'Aktionen',
          keywords: ['deafen', 'taub'],
          run: () => void session.toggleDeafen(),
        },
      ];
      if (session.inRoom) {
        actions.push({
          id: 'voice:leave',
          label: 'Channel verlassen',
          group: 'Aktionen',
          keywords: ['leave', 'auflegen'],
          run: () => void app.leaveVoice(),
        });
      }
      const people: CommandDef[] = session.participants.map((p) => ({
        id: `person:${p.identity}`,
        label: p.name,
        group: 'Leute',
        run: () => {
          shell.open('voice');
          shell.openInspector({ kind: 'person', id: p.identity });
        },
      }));
      const channels: CommandDef[] = gang.channels
        .filter((c) => c.id !== gang.myChannel)
        .map((c) => ({
          id: `channel:${c.id}`,
          label: `${c.name} beitreten`,
          group: 'Channels',
          keywords: [c.name, 'channel', 'beitreten'],
          run: () => {
            shell.open('voice');
            void gang.join(c.id);
          },
        }));
      return [...channels, ...actions, ...people];
    },
  };
}
