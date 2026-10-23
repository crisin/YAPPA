import { describe, expect, it } from 'vitest';
import { Gang } from './gang.svelte';
import { MOCK_INVITE_LINK, createMockApi } from './mockApi';
import { createMockEngine } from './mockEngine';

describe('Gang', () => {
  it('starts logged in with channels and people when a login is stored', async () => {
    const gang = new Gang(createMockApi('call', createMockEngine()));
    expect(gang.status).toBe('starting');
    await gang.start();
    expect(gang.status).toBe('in');
    expect(gang.me?.displayName).toBe('Krille');
    expect(gang.channels.map((c) => c.name)).toContain('Raid-Abend');
    expect(gang.peopleIn('raid').map((p) => p.displayName)).toEqual(['Ben', 'Lea', 'Sasha', 'Tim']);
    expect(gang.myChannel).toBeNull();
    gang.stop();
  });

  it('joins and leaves, following the presence room', async () => {
    const gang = new Gang(createMockApi('call', createMockEngine()));
    await gang.start();
    await gang.join('lobby');
    expect(gang.myChannel).toBe('lobby');
    expect(gang.peopleIn('lobby')[0].displayName).toBe('Krille');
    expect(gang.channelName('lobby')).toBe('Lobby');
    await gang.join('nope');
    expect(gang.problem).toMatch(/gibt es nicht/);
    await gang.leave();
    expect(gang.myChannel).toBeNull();
    gang.stop();
  });

  it('logs in with a link and out again', async () => {
    const gang = new Gang(createMockApi('login'));
    await gang.start();
    expect(gang.status).toBe('out');
    expect(await gang.login(MOCK_INVITE_LINK, 'X')).toBe(false);
    expect(gang.problem).toMatch(/2 bis 32/);
    expect(await gang.login(MOCK_INVITE_LINK, 'Neu')).toBe(true);
    expect(gang.status).toBe('in');
    expect(gang.people[0].displayName).toBe('Neu');
    await gang.logout();
    expect(gang.status).toBe('out');
    expect(gang.people).toEqual([]);
  });

  it('turns an unreachable server into state, not an exception', async () => {
    const gang = new Gang(createMockApi('offline'));
    await gang.start();
    expect(gang.status).toBe('out');
    expect(gang.offline).toBe(true);
  });
});
