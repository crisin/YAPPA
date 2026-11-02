// The trouble scenes (`scene.ts`) through the whole app: a flaky link, a server that is
// away, a version the server refuses, an engine that reports a problem, and a channel join
// that fails — what the banner slot, the dock and the channel tree show for each.
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from './App.svelte';
import type { Engine } from './lib/engine';
import { createMockApi } from './lib/mockApi';
import { createMockEngine, encodeInvite } from './lib/mockEngine';
import type { Scene } from './lib/scene';
import type { Event } from './lib/types/Event';

const devInvite = encodeInvite({ url: 'wss://voice.example', token: 'tok' });

async function start(scene: Scene, engine = createMockEngine(scene)) {
  const api = createMockApi(scene, engine);
  const result = render(App, { props: { api, engine } });
  return { ...result, engine, api };
}

const dock = () => screen.getByRole('region', { name: 'Sprache' });
const channels = () => screen.getByRole('list', { name: 'Voice-Channels' });
const channel = (name: RegExp) => within(channels()).getByRole('button', { name });

async function joinRaid() {
  await screen.findByRole('heading', { level: 1, name: /^Hallo / });
  await fireEvent.click(channel(/^Raid-Abend/));
  await screen.findByRole('heading', { level: 1, name: 'Raid-Abend' });
}

/** An engine whose every join is refused, the way a bad token or a dead server ends: it
 * reports the problem and lands in `disconnected`. */
function refusingJoins(mock: Engine): Engine {
  const handlers = new Set<(event: Event) => void>();
  return {
    ...mock,
    async subscribe(handler) {
      handlers.add(handler);
      const stop = await mock.subscribe(handler);
      return () => {
        handlers.delete(handler);
        stop();
      };
    },
    send(command) {
      if (command.type !== 'join') return mock.send(command);
      for (const handler of handlers) {
        handler({ type: 'connection', state: 'connecting' });
        handler({ type: 'error', message: 'Der Server hat den Beitritt abgelehnt.' });
        handler({ type: 'connection', state: 'disconnected' });
      }
      return Promise.resolve();
    },
  };
}

afterEach(() => vi.useRealTimers());

describe('Scene reconnect', () => {
  it('says the link is gone while the engine reconnects, and takes it back when it is', async () => {
    // Only the engine's clock is faked: the testing library keeps its real timeouts.
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
    await start('reconnect');
    await joinRaid();
    expect(screen.queryByRole('status', { name: '' })).not.toBeNull();
    // The mock loses the link three seconds in and has it back at five.
    vi.advanceTimersByTime(3000);
    const banner = await screen.findByText(/^Verbindung verloren – yAPPA verbindet neu\./);
    expect(banner.closest('[role]')).toHaveAttribute('role', 'status');
    expect(screen.queryByRole('button', { name: 'Hinweis schließen' })).not.toBeInTheDocument();
    expect(within(dock()).getByRole('status')).toHaveTextContent(
      'Verbindung verloren – verbinde neu …',
    );
    // Still in the room: the people stay, nobody is thrown out of the channel tree.
    expect(screen.getByRole('heading', { level: 1, name: 'Raid-Abend' })).toBeInTheDocument();
    expect(
      within(screen.getByRole('list', { name: 'Leute in Raid-Abend' })).getByText('Krille (du)'),
    ).toBeInTheDocument();

    vi.advanceTimersByTime(2000);
    await waitFor(() => expect(screen.queryByText(/^Verbindung verloren/)).not.toBeInTheDocument());
    expect(within(dock()).getByRole('status')).toHaveTextContent('Verbunden · Raid-Abend');
  });
});

describe('Scene offline', () => {
  it('shows why the server is away, offers a retry, and can be closed', async () => {
    const { api } = await start('offline');
    const text = await screen.findByText(
      'Server nicht erreichbar: keine Verbindung zu „mock.yappa.test“. Server aus? Ein laufender Call geht trotzdem weiter.',
    );
    const banner = text.closest('[role]')!;
    expect(banner).toHaveAttribute('role', 'status');
    const config = vi.spyOn(api, 'config');
    await fireEvent.click(
      within(banner as HTMLElement).getByRole('button', { name: 'Erneut versuchen' }),
    );
    expect(config).toHaveBeenCalledOnce();
    await fireEvent.click(
      within(banner as HTMLElement).getByRole('button', { name: 'Hinweis schließen' }),
    );
    expect(screen.queryByText(/Ein laufender Call geht trotzdem weiter/)).not.toBeInTheDocument();
  });

  it('carries an engine error into the banner as an alert, until it is closed', async () => {
    await start('offline');
    const card = within(await screen.findByRole('main', { name: 'Einrichtung' }));
    await fireEvent.click(
      await card.findByRole('button', { name: 'Mit neuer Einladung anmelden' }),
    );
    await fireEvent.input(card.getByLabelText('Einladung'), { target: { value: devInvite } });
    await fireEvent.click(card.getByRole('button', { name: 'Beitreten' }));
    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent('Der Server antwortet nicht.');
    await fireEvent.click(within(alert).getByRole('button', { name: 'Hinweis schließen' }));
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
});

describe('Scene update', () => {
  it('blocks the old version over the ready update, and the update shows once the block is lifted', async () => {
    await start('update');
    await screen.findByRole('heading', { level: 1, name: /^Hallo / });
    const blocked = await screen.findByRole('alert');
    expect(blocked).toHaveTextContent('Diese Version (0.1.0) ist zu alt für den Server');
    // Both banners exist; the block wins the one slot and cannot be closed.
    expect(screen.queryByText(/^Update auf 99\.0\.0 ist bereit\./)).not.toBeInTheDocument();
    expect(within(blocked).queryByRole('button', { name: 'Hinweis schließen' })).toBeNull();

    // Krille is admin: a lower minimum version from „Betrieb“ lifts the block at once.
    await fireEvent.click(
      within(screen.getByRole('navigation', { name: 'Bereiche' })).getByRole('button', {
        name: 'Admin',
      }),
    );
    await fireEvent.click(
      within(screen.getByRole('navigation', { name: 'Verwaltung' })).getByRole('button', {
        name: 'Betrieb',
      }),
    );
    await screen.findByRole('heading', { level: 1, name: 'Betrieb' });
    await fireEvent.input(screen.getByLabelText(/Mindestversion/), { target: { value: '0.1.0' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    await waitFor(() => expect(screen.queryByRole('alert')).not.toBeInTheDocument());
    const update = await screen.findByText(/^Update auf 99\.0\.0 ist bereit\./);
    expect(update.closest('[role]')).toHaveAttribute('role', 'status');
    expect(screen.getByRole('button', { name: 'Jetzt installieren' })).toBeInTheDocument();
  });
});

describe('Engine errors in a call', () => {
  it('shows what the engine reports as an alert, and a new join clears it', async () => {
    const { engine } = await start('call');
    await joinRaid();
    await engine.send({ type: 'playSound', path: 'C:/nirgends.wav', gainDb: 0 });
    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent('sound C:/nirgends.wav: cannot open');
    // The call goes on: the dock and the room are untouched.
    expect(within(dock()).getByRole('status')).toHaveTextContent('Verbunden · Raid-Abend');
    await fireEvent.click(channel(/^Lobby/));
    await screen.findByRole('heading', { level: 1, name: 'Lobby' });
    await waitFor(() => expect(screen.queryByRole('alert')).not.toBeInTheDocument());
  });
});

describe('A channel join that fails', () => {
  it('does not show me in the channel, says why, and lets me try again', async () => {
    const { engine } = await start('call', refusingJoins(createMockEngine('call')));
    await screen.findByRole('heading', { level: 1, name: /^Hallo / });
    await fireEvent.click(channel(/^Raid-Abend/));
    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent('Der Server hat den Beitritt abgelehnt.');
    // Not in there: the stage greets, the tree counts four, my name is nowhere.
    expect(screen.getByRole('heading', { level: 1, name: 'Hallo Krille' })).toBeInTheDocument();
    expect(channel(/^Raid-Abend/)).toHaveAttribute('aria-label', 'Raid-Abend, 4 Leute, beitreten');
    expect(screen.queryByText('Krille (du)')).not.toBeInTheDocument();
    expect(within(dock()).getByRole('status')).toHaveTextContent('Nicht verbunden');
    expect(within(dock()).queryByRole('button', { name: 'Verlassen' })).not.toBeInTheDocument();
    // The buttons are free again for the next try.
    expect(channel(/^Lobby/)).not.toHaveAttribute('aria-disabled');
    expect(engine).toBeDefined();
  });
});
