import { describe, expect, it } from 'vitest';
import type { PresenceMap } from './api';
import { CHANNELS, GANG, createMockApi } from './mockApi';

describe('mock API: the gang', () => {
  it('logs Krille in as admin with the four channels in order', async () => {
    const api = createMockApi();
    expect(await api.me()).toMatchObject({ id: 'krille', role: 'admin' });
    const channels = await api.channels();
    expect(channels.map((c) => c.name)).toEqual(['Lobby', 'Raid-Abend', 'Musik-Session', 'AFK']);
    expect(channels).toEqual(CHANNELS);
  });

  it('shows everybody online in the call scene, with their channel and talk mode', async () => {
    const api = createMockApi('call');
    let seen: PresenceMap = {};
    await api.presence((p) => (seen = p));
    expect(Object.keys(seen)).toHaveLength(GANG.length);
    expect(seen.lea.state).toMatchObject({ channelId: 'raid', talk: 'ptt' });
    expect(seen.rico.state.away).toBe(true);
    expect(seen.ben.displayName).toBe('Ben');
  });

  it('is quiet in the lobby scene and sees the app as Ben in the member scene', async () => {
    let seen: PresenceMap = {};
    await createMockApi('lobby').presence((p) => (seen = p));
    expect(Object.keys(seen).sort()).toEqual(['krille', 'rico']);
    expect(await createMockApi('member').me()).toMatchObject({ id: 'ben', role: 'member' });
    await expect(createMockApi('member').admin.members()).rejects.toMatchObject({
      code: 'forbidden',
    });
  });

  it('pushes my own presence change to every listener and stops after unsubscribe', async () => {
    const api = createMockApi();
    const updates: PresenceMap[] = [];
    const stop = await api.presence((p) => updates.push(p));
    await api.setPresence({ channelId: 'musik', talk: 'ptt' });
    expect(updates).toHaveLength(2);
    expect(updates[1].krille.state).toMatchObject({ channelId: 'musik', talk: 'ptt' });
    stop();
    await api.setPresence({ away: true });
    expect(updates).toHaveLength(2);
  });

  it('hands out a token per channel and refuses unknown channels', async () => {
    const api = createMockApi();
    expect(await api.voiceToken('raid')).toMatchObject({ wsUrl: 'wss://voice.yappa.example' });
    await expect(api.voiceToken('nope')).rejects.toMatchObject({ code: 'channel_unknown' });
  });

  it('redeems an invite once and rejects short names', async () => {
    const api = createMockApi();
    await expect(api.redeemInvite('gang-7f3k', 'K')).rejects.toMatchObject({
      code: 'name_too_short',
    });
    const session = await api.redeemInvite('gang-7f3k', '  Krille  ');
    expect(session.user.displayName).toBe('Krille');
    expect(session.presence.token).toContain('presence');
    await expect(api.redeemInvite('gang-7f3k', 'Krille')).rejects.toMatchObject({
      code: 'invite_invalid',
    });
  });
});

describe('mock API: scenes with trouble', () => {
  it('rejects everything as unreachable when offline', async () => {
    const api = createMockApi('offline');
    await expect(api.me()).rejects.toMatchObject({ code: 'unreachable' });
    await expect(api.channels()).rejects.toMatchObject({ code: 'unreachable' });
    await expect(api.config()).rejects.toMatchObject({ code: 'unreachable' });
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
    let seen: PresenceMap = {};
    await api.presence((p) => (seen = p));
    await api.admin.kick('tim');
    expect(seen.tim).toBeUndefined();
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
    const channels = await api.channels();
    expect(channels.map((c) => [c.name, c.position])).toEqual([
      ['Lobby', 0],
      ['Chillzone', 1],
      ['Musik-Session', 2],
      ['AFK', 3],
    ]);
    let seen: PresenceMap = {};
    await api.presence((p) => (seen = p));
    expect(seen.ben.state.channelId).toBeNull();
  });
});
