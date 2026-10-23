import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { beforeEach, describe, expect, it } from 'vitest';
import App from './App.svelte';
import { MOCK_INVITE_LINK, createMockApi } from './lib/mockApi';
import { createMockEngine, encodeInvite } from './lib/mockEngine';
import type { Scene } from './lib/scene';
import { axe } from './test-a11y';

const devInvite = encodeInvite({ url: 'wss://voice.example', token: 'tok' });

beforeEach(() => localStorage.clear());

/** The app with its own mocks in one scene; waits until the gang is loaded. */
async function start(scene: Scene = 'call') {
  const engine = createMockEngine(scene);
  const api = createMockApi(scene, engine);
  const result = render(App, { props: { api, engine } });
  if (scene === 'call' || scene === 'member' || scene === 'lobby') {
    await screen.findByRole('heading', { level: 1, name: /^Hallo / });
  } else if (scene === 'login') {
    await screen.findByRole('heading', { level: 1, name: 'Willkommen bei yAPPA' });
  }
  return result;
}

const rail = () => screen.getByRole('navigation', { name: 'Bereiche' });
const dock = () => screen.getByRole('region', { name: 'Sprache' });
const stage = () => screen.getByRole('main');
const channels = () => screen.getByRole('list', { name: 'Voice-Channels' });
const channel = (name: RegExp) => within(channels()).getByRole('button', { name });

async function joinRaid() {
  await fireEvent.click(channel(/^Raid-Abend/));
  await screen.findByRole('heading', { level: 1, name: 'Raid-Abend' });
}

async function openMe(section: string) {
  await fireEvent.click(within(rail()).getByRole('button', { name: 'Ich' }));
  await fireEvent.click(
    within(screen.getByRole('navigation', { name: 'Einstellungen' })).getByRole('button', {
      name: section,
    }),
  );
}

describe('App accessibility', () => {
  it('has no axe violations at the login, in the channel tree, in a channel and in settings', async () => {
    const login = await start('login');
    expect(await axe(login.container)).toHaveNoViolations();
    login.unmount();
    const { container } = await start();
    expect(await axe(container)).toHaveNoViolations();
    await joinRaid();
    expect(await axe(container)).toHaveNoViolations();
    await openMe('Audio und Sprechen');
    expect(await axe(container)).toHaveNoViolations();
  });
});

describe('Shell', () => {
  it('shows the rail with labels, the dock and the breadcrumb', async () => {
    await start();
    const slots = within(rail()).getAllByRole('button');
    expect(slots.map((b) => b.textContent?.trim())).toEqual(['Voice', 'Ich']);
    expect(within(rail()).getByRole('button', { name: 'Voice' })).toHaveAttribute(
      'aria-current',
      'page',
    );
    expect(within(dock()).getByRole('status')).toHaveTextContent('Nicht verbunden');
    expect(screen.getByRole('navigation', { name: 'Pfad' })).toHaveTextContent('Voice');
  });

  it('keeps the dock while another module is open, and Esc goes back', async () => {
    await start();
    await joinRaid();
    await openMe('Audio und Sprechen');
    expect(screen.getByRole('navigation', { name: 'Pfad' })).toHaveTextContent(
      'Ich / Audio und Sprechen',
    );
    expect(within(dock()).getByRole('status')).toHaveTextContent('Verbunden · Raid-Abend');
    await fireEvent.click(within(dock()).getByRole('button', { name: 'Mikrofon stummschalten' }));
    expect(within(dock()).getByText(/Mikrofon aus/)).toBeInTheDocument();
    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.getByRole('heading', { level: 1, name: 'Raid-Abend' })).toBeInTheDocument();
  });

  it('opens the palette with Strg K and joins a channel from it', async () => {
    await start();
    await fireEvent.keyDown(window, { key: 'k', ctrlKey: true });
    const palette = await screen.findByRole('dialog');
    await fireEvent.click(within(palette).getByRole('option', { name: 'Lobby beitreten' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Lobby' })).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });

  it('switches the theme and remembers it', async () => {
    await start();
    await openMe('Darstellung');
    await fireEvent.click(screen.getByRole('radio', { name: 'Hell' }));
    expect(document.documentElement.dataset.theme).toBe('light');
    expect(localStorage.getItem('yappa.theme')).toBe('light');
  });
});

describe('Voice module: channel tree', () => {
  it('shows every channel with who is in it, and nobody in a call yet', async () => {
    await start();
    const names = within(channels())
      .getAllByRole('button', { name: /beitreten|du bist hier/ })
      .map((b) => b.getAttribute('aria-label'));
    expect(names).toEqual([
      'Lobby, 2 Leute, beitreten',
      'Raid-Abend, 4 Leute, beitreten',
      'Musik-Session, 0 Leute, beitreten',
      'AFK, 1 Person, beitreten',
    ]);
    const raid = screen.getByRole('list', { name: 'Leute in Raid-Abend' });
    expect(within(raid).getByText('Lea').nextElementSibling).toHaveTextContent('PTT');
    const afk = screen.getByRole('list', { name: 'Leute in AFK' });
    expect(within(afk).getByText('abwesend')).toBeInTheDocument();
    expect(within(dock()).queryByRole('button', { name: 'Verlassen' })).not.toBeInTheDocument();
  });

  it('joins a channel with one click and shows its people as tiles', async () => {
    await start();
    await joinRaid();
    expect(within(dock()).getByRole('status')).toHaveTextContent('Verbunden · Raid-Abend');
    const tiles = within(stage()).getAllByRole('listitem');
    expect(
      tiles.map((t) => within(t).getByText(/^\w+$/, { selector: '.name' }).textContent),
    ).toEqual(['Ben', 'Sasha', 'Tim', 'Lea']);
    expect(within(tiles[2]).getByText('stumm')).toBeInTheDocument();
    const raid = screen.getByRole('list', { name: 'Leute in Raid-Abend' });
    expect(within(raid).getByText('Krille (du)')).toBeInTheDocument();
    expect(channel(/^Raid-Abend/)).toHaveAttribute('aria-current', 'true');
    expect(screen.getByRole('navigation', { name: 'Pfad' })).toHaveTextContent(
      'Voice / Raid-Abend',
    );
  });

  it('switches channels with a click and leaves from the dock', async () => {
    await start();
    await joinRaid();
    await fireEvent.click(channel(/^Lobby/));
    expect(await screen.findByRole('heading', { level: 1, name: 'Lobby' })).toBeInTheDocument();
    expect(
      within(screen.getByRole('list', { name: 'Leute in Lobby' })).getByText('Krille (du)'),
    ).toBeInTheDocument();
    await fireEvent.click(within(dock()).getByRole('button', { name: 'Verlassen' }));
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Hallo Krille' }),
    ).toBeInTheDocument();
    expect(screen.queryByText('Krille (du)')).not.toBeInTheDocument();
  });

  it('opens a person of my channel in the inspector and closes it with Esc', async () => {
    await start();
    await joinRaid();
    const raid = screen.getByRole('list', { name: 'Leute in Raid-Abend' });
    await fireEvent.click(within(raid).getByRole('button', { name: 'Ben' }));
    const details = screen.getByRole('complementary', { name: 'Details' });
    expect(within(details).getByRole('heading', { name: 'Ben' })).toBeInTheDocument();
    await fireEvent.click(within(details).getByRole('button', { name: 'Für mich stummschalten' }));
    expect(within(details).getByRole('button', { name: 'Für mich stumm' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.queryByRole('complementary', { name: 'Details' })).not.toBeInTheDocument();
  });

  it('mutes one person for me only, on the tile', async () => {
    await start();
    await joinRaid();
    const ben = within(stage()).getAllByRole('listitem')[0];
    await fireEvent.click(within(ben).getByRole('button', { name: 'Hörbar' }));
    expect(within(ben).getByRole('button', { name: 'Stumm für mich' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
  });

  it('toggles mute in the dock', async () => {
    await start();
    const mute = within(dock()).getByRole('button', { name: 'Mikrofon stummschalten' });
    expect(mute).toHaveAttribute('aria-pressed', 'false');
    await fireEvent.click(mute);
    expect(mute).toHaveAttribute('aria-pressed', 'true');
  });
});

describe('Voice module: the way in', () => {
  it('asks for a name when an invite link is pasted and lands in the channel tree', async () => {
    await start('login');
    expect(within(dock()).getByRole('meter', { name: 'Mikrofonpegel' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Beitreten' })).toBeDisabled();
    await fireEvent.input(screen.getByLabelText('Einladung'), {
      target: { value: MOCK_INVITE_LINK },
    });
    const name = screen.getByLabelText('Dein Name');
    await fireEvent.input(name, { target: { value: 'K' } });
    expect(screen.getByRole('button', { name: 'Beitreten' })).toBeDisabled();
    await fireEvent.input(name, { target: { value: 'Neu' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Beitreten' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Hallo Neu' })).toBeInTheDocument();
    expect(channel(/^Raid-Abend/)).toBeInTheDocument();
  });

  it('says why a used or wrong invite does not work', async () => {
    await start('login');
    await fireEvent.input(screen.getByLabelText('Einladung'), {
      target: { value: 'yappa://invite/falsch?server=https://x' },
    });
    await fireEvent.input(screen.getByLabelText('Dein Name'), { target: { value: 'Neu' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Beitreten' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('ungültig');
  });

  it('explains a pasted text that is not an invite', async () => {
    await start('login');
    await fireEvent.input(screen.getByLabelText('Einladung'), { target: { value: 'hallo' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Beitreten' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('keine yAPPA-Einladung');
  });

  it('still takes a developer invite straight into its room', async () => {
    await start('login');
    await fireEvent.input(screen.getByLabelText('Einladung'), { target: { value: devInvite } });
    expect(screen.queryByLabelText('Dein Name')).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Beitreten' }));
    await screen.findByRole('heading', { level: 1, name: 'gang' });
    const people = within(stage()).getAllByRole('listitem');
    expect(within(people[0]).getByText('Anna')).toBeInTheDocument();
    await fireEvent.click(within(dock()).getByRole('button', { name: 'Verlassen' }));
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Willkommen bei yAPPA' }),
    ).toBeInTheDocument();
  });
});

describe('Ich module', () => {
  it('shows the account and logs out after asking', async () => {
    await start();
    await openMe('Konto');
    expect(screen.getByText('Krille')).toBeInTheDocument();
    expect(screen.getByText('Admin')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Abmelden …' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Ja, abmelden' }));
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Willkommen bei yAPPA' }),
    ).toBeInTheDocument();
  });

  it('has the devices with a live microphone test', async () => {
    await start();
    await openMe('Audio und Sprechen');
    expect(await screen.findByRole('option', { name: 'Webcam' })).toBeInTheDocument();
    expect(screen.getByRole('meter', { name: 'Mikrofontest' })).toBeInTheDocument();
  });

  it('shows the diagnosis with the timeline and exports it', async () => {
    await start();
    await joinRaid();
    await openMe('Diagnose');
    const panel = screen.getByRole('region', { name: 'Diagnose' });
    expect(within(panel).getByText('verbunden')).toBeInTheDocument();
    expect(within(panel).getByText('raid')).toBeInTheDocument();
    expect(within(panel).getByText(/joining wss:\/\/voice\.yappa\.example/)).toBeInTheDocument();
    await fireEvent.click(within(panel).getByRole('button', { name: 'Diagnose-Datei speichern' }));
    expect(await within(panel).findByText(/Gespeichert:/)).toBeInTheDocument();
  });

  it('shows the help and goes back on Esc', async () => {
    await start();
    await joinRaid();
    await openMe('Hilfe');
    expect(screen.getByRole('heading', { level: 1, name: 'Mittesten' })).toBeInTheDocument();
    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.getByRole('heading', { level: 1, name: 'Raid-Abend' })).toBeInTheDocument();
  });

  it('explains a diagnosis value with a tooltip that leads into the glossary', async () => {
    await start();
    await joinRaid();
    await openMe('Diagnose');
    await fireEvent.click(
      await screen.findByRole('button', { name: 'Erklärung: Jitter' }, { timeout: 3000 }),
    );
    expect(screen.getByRole('tooltip')).toHaveTextContent('Jitter');
    await fireEvent.click(screen.getByRole('button', { name: 'Mehr im Glossar' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Glossar' })).toBeInTheDocument();
    expect(
      within(screen.getByRole('navigation', { name: 'Einstellungen' })).getByRole('button', {
        name: 'Hilfe',
      }),
    ).toHaveAttribute('aria-current', 'page');
  });
});
