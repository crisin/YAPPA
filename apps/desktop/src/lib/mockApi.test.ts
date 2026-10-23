import { describe, expect, it } from 'vitest';
import type { PresenceEntry } from './api';
import { inviteKind } from './api';
import { createMockEngine } from './mockEngine';
import { CHANNELS, GANG, MOCK_INVITE_LINK, createMockApi } from './mockApi';
import type { Event } from './types/Event';

async function people(api = createMockApi()): Promise<PresenceEntry[]> {
  let seen: PresenceEntry[] = [];
  await api.presence((p) => (seen = p));
  return seen;
}

describe('mock API: the gang', () => {
  it('resumes Krille as admin, with the four channels in order', async () => {
    const api = createMockApi();
    expect(await api.resume()).toMatchObject({ id: 'krille', role: 'admin' });
    expect(await api.channels()).toEqual(CHANNELS);
  });

  it('lists everybody online, me first and nowhere yet, the others by name', async () => {
    const seen = await people();
    expect(seen).toHaveLength(GANG.length);
    expect(seen[0]).toMatchObject({ userId: 'krille', state: { channelId: null } });
    expect(seen.slice(1).map((p) => p.displayName)).toEqual([
      'Ben',
      'Jule',
      'Lea',
      'Mo',
      'Rico',
      'Sasha',
      'Tim',
    ]);
    expect(seen.find((p) => p.userId === 'lea')?.state).toMatchObject({
      channelId: 'raid',
      talk: 'ptt',
    });
  });

  it('is quiet in the lobby scene and sees the app as Ben in the member scene', async () => {
    expect((await people(createMockApi('lobby'))).map((p) => p.userId)).toEqual(['krille', 'rico']);
    expect(await createMockApi('member').resume()).toMatchObject({ id: 'ben', role: 'member' });
    await expect(createMockApi('member').admin.members()).rejects.toMatchObject({
      code: 'forbidden',
    });
  });

  it('joins a channel through the engine and moves me there in the presence', async () => {
    const engine = createMockEngine();
    const events: Event[] = [];
    await engine.subscribe((e) => events.push(e));
    const api = createMockApi('call', engine);
    const updates: PresenceEntry[][] = [];
    const stop = await api.presence((p) => updates.push(p));
    await api.joinChannel('raid');
    expect(events).toContainEqual({ type: 'joined', room: 'raid', identity: 'krille' });
    const participants = events.find((e) => e.type === 'participants');
    expect(participants).toMatchObject({
      participants: [
        { name: 'Ben' },
        { name: 'Sasha' },
        { name: 'Tim', muted: true },
        { name: 'Lea' },
      ],
    });
    expect(updates.at(-1)?.[0].state.channelId).toBe('raid');
    await api.leaveChannel();
    expect(updates.at(-1)?.[0].state.channelId).toBeNull();
    expect(events.filter((e) => e.type !== 'log').at(-1)).toMatchObject({
      type: 'connection',
      state: 'disconnected',
    });
    stop();
    await api.setPresence({ away: true });
    expect(updates).toHaveLength(3);
    await expect(api.joinChannel('nope')).rejects.toMatchObject({ code: 'channel_unknown' });
  });

  it('logs in once with an invite link and checks the name', async () => {
    const api = createMockApi('login');
    expect(await api.resume()).toBeNull();
    expect(await people(api)).not.toContainEqual(expect.objectContaining({ userId: 'krille' }));
    await expect(api.redeem('hallo', 'Krille')).rejects.toMatchObject({ code: 'invite_invalid' });
    await expect(api.redeem(MOCK_INVITE_LINK, 'K')).rejects.toMatchObject({
      code: 'name_invalid',
    });
    const me = await api.redeem(` ${MOCK_INVITE_LINK}\n`, '  Krille  ');
    expect(me.displayName).toBe('Krille');
    expect(await api.resume()).toMatchObject({ id: 'krille' });
    await expect(api.redeem(MOCK_INVITE_LINK, 'Krille')).rejects.toMatchObject({
      code: 'invite_invalid',
    });
    await api.logout();
    expect(await api.resume()).toBeNull();
  });

  it('tells invite links from developer invites', () => {
    expect(inviteKind(MOCK_INVITE_LINK)).toBe('link');
    expect(inviteKind(' yappa1.abc ')).toBe('direct');
    expect(inviteKind('hallo')).toBeNull();
  });
});

describe('mock API: scenes with trouble', () => {
  it('rejects as unreachable when offline', async () => {
    const api = createMockApi('offline');
    await expect(api.resume()).rejects.toMatchObject({ code: 'unreachable' });
    await expect(api.channels()).rejects.toMatchObject({ code: 'unreachable' });
    await expect(api.joinChannel('lobby')).rejects.toMatchObject({ code: 'unreachable' });
  });

  it('demands a newer version in the update scene', async () => {
    expect((await createMockApi('update').config()).minVersion).toBe('99.0.0');
    expect((await createMockApi('call').config()).minVersion).toBe('0.1.0');
  });
});

describe('mock API: admin', () => {
  it('lists members with their client, creates and revokes invites', async () => {
    const api = createMockApi();
    const members = await api.admin.members();
    expect(members).toHaveLength(GANG.length);
    expect(members.find((m) => m.user.id === 'rico')?.client?.appVersion).toBe('0.0.9');
    const invite = await api.admin.createInvite();
    expect((await api.admin.invites()).map((i) => i.code)).toContain(invite.code);
    await api.admin.revokeInvite(invite.code);
    expect((await api.admin.invites()).map((i) => i.code)).not.toContain(invite.code);
  });

  it('kicks someone out of presence and logs it', async () => {
    const api = createMockApi();
    let seen: PresenceEntry[] = [];
    await api.presence((p) => (seen = p));
    await api.admin.kick('tim');
    expect(seen.map((p) => p.userId)).not.toContain('tim');
    expect((await api.admin.events())[0]).toMatchObject({ kind: 'kicked', userId: 'tim' });
  });

  it('creates, moves, renames and deletes channels and keeps positions dense', async () => {
    const api = createMockApi();
    const created = await api.admin.createChannel('Chill Zone');
    expect(created).toMatchObject({ id: 'chill-zone', position: 4 });
    await api.admin.moveChannel('chill-zone', 1);
    expect((await api.channels()).map((c) => c.id)).toEqual([
      'lobby',
      'chill-zone',
      'raid',
      'musik',
      'afk',
    ]);
    await api.admin.renameChannel('chill-zone', 'Chillzone');
    await api.admin.deleteChannel('raid');
    expect((await api.channels()).map((c) => [c.name, c.position])).toEqual([
      ['Lobby', 0],
      ['Chillzone', 1],
      ['Musik-Session', 2],
      ['AFK', 3],
    ]);
    expect((await people(api)).find((p) => p.userId === 'ben')?.state.channelId).toBeNull();
  });
});
