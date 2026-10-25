import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { beforeEach, describe, expect, it } from 'vitest';
import App from './App.svelte';
import { MOCK_INVITE_LINK, createMockApi } from './lib/mockApi';
import { createMockEngine, encodeInvite } from './lib/mockEngine';
import type { Scene } from './lib/scene';
import { axe } from './test-a11y';

const devInvite = encodeInvite({ url: 'wss://voice.example', token: 'tok' });

beforeEach(() => localStorage.clear());

/** The app with its own mocks in one scene; waits until the gang is loaded. `prepare` may
 * change the mocks before the first render (settings as an older install left them). */
async function start(
  scene: Scene = 'call',
  prepare?: (api: ReturnType<typeof createMockApi>) => Promise<void>,
) {
  const engine = createMockEngine(scene);
  const api = createMockApi(scene, engine);
  await prepare?.(api);
  const result = render(App, { props: { api, engine } });
  if (scene === 'call' || scene === 'member' || scene === 'lobby') {
    await screen.findByRole('heading', { level: 1, name: /^Hallo / });
  } else if (scene === 'login') {
    await screen.findByRole('heading', { level: 1, name: 'Willkommen bei yAPPA' });
  }
  return { ...result, engine, api };
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

const meNav = () => screen.getByRole('navigation', { name: 'Einstellungen' });

/** Opens a section of „Ich“; a Profi-only one switches the view first. */
async function openMe(section: string) {
  await fireEvent.click(within(rail()).getByRole('button', { name: 'Ich' }));
  if (!within(meNav()).queryByRole('button', { name: section })) {
    await fireEvent.click(within(meNav()).getByRole('radio', { name: 'Profi' }));
  }
  await fireEvent.click(await within(meNav()).findByRole('button', { name: section }));
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
    expect(slots.map((b) => b.textContent?.trim())).toEqual(['Voice', 'Admin', 'Ich']);
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

describe('Mute and deafen (A3)', () => {
  it('shows stumm and taub from the presence and follows the engine for me', async () => {
    const engine = createMockEngine('call');
    const api = createMockApi('call', engine);
    render(App, { props: { api, engine } });
    await screen.findByRole('heading', { level: 1, name: /^Hallo / });
    const raid = screen.getByRole('list', { name: 'Leute in Raid-Abend' });
    expect(within(raid).getByText('Tim').nextElementSibling).toHaveTextContent('stumm');
    // A key press in a game: the engine flips and reports; the dock shows it.
    await engine.send({ type: 'toggleDeafened' });
    await waitFor(() =>
      expect(within(dock()).getByRole('button', { name: 'Ton stummschalten' })).toHaveAttribute(
        'aria-pressed',
        'true',
      ),
    );
    expect(within(dock()).getByRole('button', { name: 'Mikrofon stummschalten' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    expect(within(dock()).getByText(/Mikrofon aus · Ton aus/)).toBeInTheDocument();
    await engine.send({ type: 'toggleDeafened' });
    await waitFor(() =>
      expect(
        within(dock()).getByRole('button', { name: 'Mikrofon stummschalten' }),
      ).toHaveAttribute('aria-pressed', 'false'),
    );
  });
});

describe('Voice module: the way in', () => {
  it('asks for a name, then microphone and speaking mode, and lands in the lobby', async () => {
    const { api } = await start('login');
    expect(screen.getByRole('list', { name: 'Schritte' })).toBeInTheDocument();
    expect(screen.queryByRole('navigation', { name: 'Bereiche' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Beitreten' })).toBeDisabled();
    await fireEvent.input(screen.getByLabelText('Einladung'), {
      target: { value: MOCK_INVITE_LINK },
    });
    const name = screen.getByLabelText('Dein Name');
    await fireEvent.input(name, { target: { value: 'K' } });
    expect(screen.getByRole('button', { name: 'Beitreten' })).toBeDisabled();
    await fireEvent.input(name, { target: { value: 'Neu' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Beitreten' }));

    expect(
      await screen.findByRole('heading', { level: 1, name: 'Hört dich yAPPA?' }),
    ).toBeVisible();
    expect(screen.getByText(/Hallo Neu!/)).toBeInTheDocument();
    expect(screen.getByRole('meter', { name: 'Mikrofontest' })).toBeInTheDocument();
    expect(screen.getByLabelText('Ausgabe (Kopfhörer oder Lautsprecher)')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Weiter' }));

    expect(screen.getByRole('heading', { level: 1, name: 'Wie redest du?' })).toBeInTheDocument();
    const done = screen.getByRole('button', { name: 'Ab in die Lobby' });
    expect(done).toBeDisabled();
    await fireEvent.click(screen.getByRole('radio', { name: /^Lautsprecher/ }));
    await waitFor(() => expect(screen.getByRole('radio', { name: /^Taste halten/ })).toBeChecked());
    expect(screen.getByRole('button', { name: /Taste/ })).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('radio', { name: /^Headset/ }));
    await waitFor(() => expect(screen.getByRole('radio', { name: /^Automatisch/ })).toBeChecked());
    // Remembered for „Mich selbst hören“ (A5).
    await waitFor(async () => expect((await api.appSettings()).listening).toBe('headset'));
    await fireEvent.click(done);

    expect(await screen.findByRole('heading', { level: 1, name: 'Lobby' })).toBeInTheDocument();
    expect(channel(/^Raid-Abend/)).toBeInTheDocument();
  });

  it('can go back from the speaking mode to the microphone, accessibly', async () => {
    const { container } = await start('login');
    await fireEvent.input(screen.getByLabelText('Einladung'), {
      target: { value: MOCK_INVITE_LINK },
    });
    await fireEvent.input(screen.getByLabelText('Dein Name'), { target: { value: 'Neu' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Beitreten' }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Weiter' }));
    await fireEvent.click(screen.getByRole('radio', { name: /^Headset/ }));
    expect(await axe(container)).toHaveNoViolations();
    await fireEvent.click(screen.getByRole('button', { name: 'Zurück' }));
    expect(await axe(container)).toHaveNoViolations();
    expect(screen.getByRole('heading', { level: 1, name: 'Hört dich yAPPA?' })).toBeInTheDocument();
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
    expect(within(stage()).getByText('Krille')).toBeInTheDocument();
    expect(within(stage()).getByText('Admin')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Abmelden …' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Ja, abmelden' }));
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Willkommen bei yAPPA' }),
    ).toBeInTheDocument();
  });

  it('sets how the microphone goes on air, with a push-to-talk key', async () => {
    await start();
    await openMe('Audio und Sprechen');
    expect(screen.getByRole('radio', { name: /Automatisch/ })).toBeChecked();
    expect(screen.getByLabelText(/Ab hier geht das Mikro an/)).toHaveValue('-50');
    await fireEvent.click(screen.getByRole('radio', { name: /Taste halten/ }));
    expect(
      await screen.findByText('Ohne Taste geht dein Mikro in diesem Modus nie an.'),
    ).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Taste festlegen' }));
    expect(await screen.findByText('Maustaste 4')).toBeInTheDocument();
    expect(screen.queryByLabelText(/Ab hier geht das Mikro an/)).not.toBeInTheDocument();
    expect(screen.getByLabelText('Rauschunterdrückung')).toHaveValue('deepFilter');
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
    // Escape closes the tooltip without stepping the shell back.
    await fireEvent.keyDown(screen.getByRole('button', { name: 'Erklärung: Jitter' }), {
      key: 'Escape',
    });
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
    expect(screen.getByRole('navigation', { name: 'Pfad' })).toHaveTextContent('Ich / Diagnose');
    await fireEvent.click(screen.getByRole('button', { name: 'Erklärung: Jitter' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Mehr im Glossar' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Glossar' })).toBeInTheDocument();
    expect(
      within(screen.getByRole('navigation', { name: 'Einstellungen' })).getByRole('button', {
        name: 'Hilfe',
      }),
    ).toHaveAttribute('aria-current', 'page');
  });
});

describe('Settings: Einfach und Profi', () => {
  const sections = () =>
    within(meNav())
      .getAllByRole('button')
      .map((b) => b.textContent?.trim());

  it('shows four everyday sections in Einfach and adds the rest in Profi, stored', async () => {
    const engine = createMockEngine('call');
    const api = createMockApi('call', engine);
    render(App, { props: { api, engine } });
    await screen.findByRole('heading', { level: 1, name: /^Hallo / });
    await fireEvent.click(within(rail()).getByRole('button', { name: 'Ich' }));
    expect(sections()).toEqual([
      'Audio und Sprechen',
      'Darstellung',
      'Benachrichtigungen',
      'Konto',
      'Hilfe',
    ]);
    await fireEvent.click(within(meNav()).getByRole('radio', { name: 'Profi' }));
    await waitFor(() => expect(sections()).toContain('Signalkette'));
    expect(sections()).toEqual(
      expect.arrayContaining(['Tasten', 'Netzwerk', 'Updates', 'Diagnose']),
    );
    expect((await api.appSettings()).view).toBe('pro');
  });

  it('goes back to „Audio und Sprechen“ when Einfach hides the open section', async () => {
    await start();
    await openMe('Netzwerk');
    expect(screen.getByRole('heading', { level: 1, name: 'Netzwerk' })).toBeInTheDocument();
    await fireEvent.click(within(meNav()).getByRole('radio', { name: 'Einfach' }));
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Audio und Sprechen' }),
    ).toBeInTheDocument();
  });

  it('offers push-to-talk, mute and deafen keys in „Tasten“ and refuses a double key', async () => {
    const engine = createMockEngine('call');
    const api = createMockApi('call', engine);
    render(App, { props: { api, engine } });
    await screen.findByRole('heading', { level: 1, name: /^Hallo / });
    await openMe('Tasten');
    expect(screen.getByRole('heading', { level: 1, name: 'Tasten' })).toBeInTheDocument();
    const section = (name: string) => screen.getByRole('region', { name: new RegExp(`^${name}`) });
    for (const name of ['Push-to-Talk', 'Stumm', 'Taub']) {
      expect(within(section(name)).getByText('keine')).toBeInTheDocument();
    }
    await fireEvent.click(
      within(section('Stumm')).getByRole('button', { name: 'Taste festlegen' }),
    );
    expect(await within(section('Stumm')).findByText('F7')).toBeInTheDocument();
    expect((await api.appSettings()).muteKey).toBe('F7');
    // The mock presses F7 for „Stumm“ and F8 for „Taub“. Once F8 belongs to „Stumm“ (set
    // by hand), choosing the „Taub“ key again runs into the one-key-one-thing rule.
    await fireEvent.click(within(section('Taub')).getByRole('button', { name: 'Taste festlegen' }));
    expect(await within(section('Taub')).findByText('F8')).toBeInTheDocument();
    await api.setAppSettings({ ...(await api.appSettings()), muteKey: 'F8' });
    await fireEvent.click(within(section('Taub')).getByRole('button', { name: 'Andere Taste' }));
    expect(await within(section('Taub')).findByRole('alert')).toHaveTextContent(
      'schon für „Stumm“ belegt',
    );
    await fireEvent.click(within(section('Taub')).getByRole('button', { name: 'Entfernen' }));
    await waitFor(async () => expect((await api.appSettings()).deafenKey).toBeNull());
    expect(within(section('Taub')).getByText('keine')).toBeInTheDocument();
  });

  it('shows the send chain as stages with levels, cost and parameters', async () => {
    const { container } = await start();
    await openMe('Signalkette');
    const stages = screen.getByRole('list', { name: 'Stufen der Signalkette' });
    expect(within(stages).getAllByRole('button')).toHaveLength(7);
    expect(within(stages).getAllByRole('meter')).toHaveLength(7);
    expect(await screen.findByText('Rechenzeit je 10-ms-Block')).toBeInTheDocument();
    await waitFor(() => expect(screen.getByText('10 ms')).toBeInTheDocument(), { timeout: 3000 });

    const compressor = within(stages).getByRole('button', { name: /^Kompressor/ });
    await fireEvent.click(compressor);
    expect(compressor).toHaveAttribute('aria-expanded', 'true');
    expect(compressor).toHaveTextContent('3:1');
    await fireEvent.click(screen.getByRole('checkbox', { name: /An – gleicht laut und leise an/ }));
    await waitFor(() => expect(compressor).toHaveTextContent('aus'));

    const eq = within(stages).getByRole('button', { name: /^EQ/ });
    await fireEvent.click(eq);
    await fireEvent.click(screen.getByRole('button', { name: 'Band hinzufügen' }));
    await waitFor(() => expect(eq).toHaveTextContent('1 Band'));
    expect(await axe(container)).toHaveNoViolations();
  });

  it('hears myself while the Signalkette is open and stops the moment it closes (A5)', async () => {
    const { engine } = await start();
    const seen: boolean[] = [];
    await engine.subscribe((event) => {
      if (event.type === 'monitorState') seen.push(event.on);
    });
    await openMe('Signalkette');
    const monitor = await screen.findByRole('checkbox', { name: 'Mich selbst hören' });
    await waitFor(() => expect(monitor).toBeChecked());
    expect(await screen.findByText(/etwa 50 ms Mikro bis Kopfhörer/)).toBeInTheDocument();
    expect(screen.queryByRole('note')).not.toBeInTheDocument();
    await fireEvent.click(monitor);
    await waitFor(() => expect(monitor).not.toBeChecked());
    expect(screen.queryByText(/Mikro bis Kopfhörer/)).not.toBeInTheDocument();
    await fireEvent.click(monitor);
    await waitFor(() => expect(monitor).toBeChecked());
    await openMe('Audio und Sprechen');
    expect(seen.at(-1)).toBe(false);
    // The wish survives within the run: open again, on again.
    await openMe('Signalkette');
    await waitFor(() =>
      expect(screen.getByRole('checkbox', { name: 'Mich selbst hören' })).toBeChecked(),
    );
    await openMe('Diagnose');
    expect(await screen.findByText('Mich selbst hören')).toBeInTheDocument();
    expect(screen.getByText('aus')).toBeInTheDocument();
  });

  it('warns on speakers and does not start hearing myself by itself (A5)', async () => {
    const { container } = await start('call', async (api) =>
      api.setAppSettings({ ...(await api.appSettings()), listening: 'speakers' }),
    );
    await openMe('Signalkette');
    const monitor = await screen.findByRole('checkbox', { name: 'Mich selbst hören' });
    expect(monitor).not.toBeChecked();
    expect(screen.getByRole('note')).toHaveTextContent('Rückkopplung');
    expect(screen.queryByText(/Mikro bis Kopfhörer/)).not.toBeInTheDocument();
    await fireEvent.click(monitor);
    await waitFor(() => expect(monitor).toBeChecked());
    expect(await axe(container)).toHaveNoViolations();
  });
});

describe('Admin module', () => {
  const adminNav = () => screen.getByRole('navigation', { name: 'Verwaltung' });
  async function openAdmin(section: string) {
    await fireEvent.click(within(rail()).getByRole('button', { name: 'Admin' }));
    await fireEvent.click(within(adminNav()).getByRole('button', { name: section }));
    await screen.findByRole('heading', { level: 1, name: section });
  }

  it('is only in the rail for admins', async () => {
    await start('member');
    expect(within(rail()).queryByRole('button', { name: 'Admin' })).not.toBeInTheDocument();
  });

  it('lists the members with role, place and version, and kicks after a second click', async () => {
    const { container } = await start();
    await openAdmin('Leute');
    const row = (await screen.findByRole('rowheader', { name: 'Rico' })).closest('tr')!;
    expect(row).toHaveTextContent('in AFK · abwesend');
    expect(row).toHaveTextContent('0.0.9');
    expect(screen.getByRole('rowheader', { name: 'Krille (du)' })).toBeInTheDocument();
    expect(screen.getByRole('combobox', { name: 'Rolle von Krille' })).toBeDisabled();
    await fireEvent.change(screen.getByRole('combobox', { name: 'Rolle von Ben' }), {
      target: { value: 'admin' },
    });
    await waitFor(() =>
      expect(screen.getByRole('combobox', { name: 'Rolle von Ben' })).toHaveValue('admin'),
    );
    const kick = within(row).getByRole('button', { name: 'Rauswerfen' });
    await fireEvent.click(kick);
    expect(kick).toHaveTextContent('Wirklich rauswerfen?');
    await fireEvent.click(kick);
    await waitFor(() => expect(row).toHaveTextContent('offline'));
    expect(await axe(container)).toHaveNoViolations();
  });

  it('creates an invite link to copy and withdraws an open one', async () => {
    await start();
    await openAdmin('Einladungen');
    await fireEvent.click(screen.getByRole('button', { name: 'Neue Einladung' }));
    const link = await screen.findByLabelText(/Neuer Link/);
    expect((link as HTMLInputElement).value).toMatch(/^yappa:\/\/invite\/gang-\d{4}\?server=/);
    const code = (link as HTMLInputElement).value.split('/')[3].split('?')[0];
    await fireEvent.click(screen.getByRole('button', { name: `Einladung ${code} zurückziehen` }));
    await waitFor(() => expect(screen.queryByText(code)).not.toBeInTheDocument());
  });

  it('adds, renames, moves and deletes channels, and the channel tree follows', async () => {
    await start();
    await openAdmin('Channels');
    await fireEvent.input(screen.getByLabelText('Neuer Channel'), {
      target: { value: 'Strategie' },
    });
    await fireEvent.click(screen.getByRole('button', { name: 'Anlegen' }));
    const order = () =>
      within(screen.getByRole('list', { name: 'Channel-Reihenfolge' }))
        .getAllByRole('listitem')
        .map((li) => li.querySelector('.name')?.textContent);
    await waitFor(() => expect(order().at(-1)).toBe('Strategie'));

    await fireEvent.click(screen.getByRole('button', { name: 'Strategie nach oben' }));
    await waitFor(() => expect(order().at(-2)).toBe('Strategie'));

    const strategy = screen.getByText('Strategie').closest('li')!;
    await fireEvent.click(within(strategy).getByRole('button', { name: 'Umbenennen' }));
    const input = screen.getByLabelText('Neuer Name für Strategie');
    await fireEvent.input(input, { target: { value: 'Taktik' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    await waitFor(() => expect(order()).toContain('Taktik'));

    await fireEvent.click(screen.getByRole('button', { name: 'Taktik löschen' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Taktik wirklich löschen?' }));
    await waitFor(() => expect(order()).not.toContain('Taktik'));
  });

  it('sets the minimum version, warns before locking out my own app, and shows the log', async () => {
    await start();
    await openAdmin('Betrieb');
    const min = screen.getByLabelText(/Mindestversion/);
    await fireEvent.input(min, { target: { value: 'quatsch' } });
    expect(screen.getByText(/Zahlen wie 0.2.0/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Speichern' })).toBeDisabled();
    await fireEvent.input(min, { target: { value: '0.1.0' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    expect(await screen.findByText(/Gespeichert/)).toBeInTheDocument();
    expect(screen.getByRole('list', { name: 'Ereignis-Log' })).toBeInTheDocument();
  });
});

describe('Server away at start', () => {
  it('offers „Erneut versuchen“ instead of the login, and the login on request', async () => {
    await start('offline');
    // The banner slot may carry its own „Erneut versuchen“ (config); the card is in main.
    const main = await screen.findByRole('main', { name: 'Einrichtung' });
    const card = () => within(main);
    expect(
      await card().findByRole('heading', { level: 1, name: 'Server nicht erreichbar' }),
    ).toBeInTheDocument();
    expect(card().queryByLabelText('Einladung')).not.toBeInTheDocument();
    // The reason from the client, without the prefix the heading already says.
    expect(card().getByRole('status')).toHaveTextContent(/^keine Verbindung zu „mock.yappa.test“/);
    await fireEvent.click(card().getByRole('button', { name: 'Erneut versuchen' }));
    expect(
      await card().findByRole('heading', { level: 1, name: 'Server nicht erreichbar' }),
    ).toBeInTheDocument();
    await fireEvent.click(card().getByRole('button', { name: 'Mit neuer Einladung anmelden' }));
    expect(card().getByLabelText('Einladung')).toBeInTheDocument();
  });
});

describe('Invite links from the OS', () => {
  it('puts a clicked invite link into the login card', async () => {
    await start('invite');
    await waitFor(() => expect(screen.getByLabelText('Einladung')).toHaveValue(MOCK_INVITE_LINK));
    expect(screen.getByLabelText('Dein Name')).toBeInTheDocument();
  });
});
