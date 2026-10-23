import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { beforeEach, describe, expect, it } from 'vitest';
import App from './App.svelte';
import { encodeInvite } from './lib/mockEngine';
import { axe } from './test-a11y';

const invite = encodeInvite({ url: 'wss://voice.example', token: 'tok' });

beforeEach(() => localStorage.clear());

async function join() {
  await fireEvent.input(screen.getByLabelText('Einladung'), { target: { value: invite } });
  await fireEvent.click(screen.getByRole('button', { name: 'Beitreten' }));
  await screen.findByRole('heading', { level: 1, name: 'gang' });
}

const rail = () => screen.getByRole('navigation', { name: 'Bereiche' });
const dock = () => screen.getByRole('region', { name: 'Sprache' });
const stage = () => screen.getByRole('main');

async function openMe(section: string) {
  await fireEvent.click(within(rail()).getByRole('button', { name: 'Ich' }));
  await fireEvent.click(
    within(screen.getByRole('navigation', { name: 'Einstellungen' })).getByRole('button', {
      name: section,
    }),
  );
}

describe('App accessibility', () => {
  it('has no axe violations in the join card, the room and the settings', async () => {
    const { container } = render(App);
    await screen.findByRole('heading', { level: 1, name: 'Beitreten' });
    expect(await axe(container)).toHaveNoViolations();
    await join();
    expect(await axe(container)).toHaveNoViolations();
    await openMe('Audio und Sprechen');
    expect(await axe(container)).toHaveNoViolations();
  });
});

describe('Shell', () => {
  it('shows the rail with labels, the dock and the breadcrumb', async () => {
    render(App);
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
    render(App);
    await join();
    await openMe('Audio und Sprechen');
    expect(screen.getByRole('navigation', { name: 'Pfad' })).toHaveTextContent(
      'Ich / Audio und Sprechen',
    );
    expect(within(dock()).getByRole('status')).toHaveTextContent('Verbunden · gang');
    await fireEvent.click(within(dock()).getByRole('button', { name: 'Mikrofon stummschalten' }));
    expect(within(dock()).getByText(/Mikrofon aus/)).toBeInTheDocument();
    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.getByRole('heading', { level: 1, name: 'gang' })).toBeInTheDocument();
  });

  it('opens the palette with Strg K and runs a command from it', async () => {
    render(App);
    await fireEvent.keyDown(window, { key: 'k', ctrlKey: true });
    const palette = await screen.findByRole('dialog');
    await fireEvent.click(within(palette).getByRole('option', { name: 'Darstellung' }));
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Darstellung' }),
    ).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });

  it('switches the theme and remembers it', async () => {
    render(App);
    await openMe('Darstellung');
    await fireEvent.click(screen.getByRole('radio', { name: 'Hell' }));
    expect(document.documentElement.dataset.theme).toBe('light');
    expect(localStorage.getItem('yappa.theme')).toBe('light');
  });
});

describe('Voice module', () => {
  it('starts with the join card and no channel in the sidebar', async () => {
    render(App);
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('Beitreten');
    expect(screen.getByRole('button', { name: 'Beitreten' })).toBeDisabled();
    expect(screen.getByText(/Noch in keinem Channel/)).toBeInTheDocument();
    expect(within(dock()).getByRole('meter', { name: 'Mikrofonpegel' })).toBeInTheDocument();
  });

  it('explains a pasted text that is not an invite', async () => {
    render(App);
    await fireEvent.input(screen.getByLabelText('Einladung'), { target: { value: 'hallo' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Beitreten' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('keine yAPPA-Einladung');
    expect(within(dock()).getByRole('status')).toHaveTextContent('Nicht verbunden');
  });

  it('joins with an invite and shows who is there, in the stage and the sidebar', async () => {
    render(App);
    await join();
    expect(within(dock()).getByRole('status')).toHaveTextContent('Verbunden');
    const people = within(stage()).getAllByRole('listitem');
    expect(people).toHaveLength(2);
    expect(within(people[0]).getByText('Anna')).toBeInTheDocument();
    expect(within(people[1]).getByText('stumm')).toBeInTheDocument();
    expect(screen.getByRole('slider', { name: 'Lautstärke Anna' })).toHaveValue('0');
    const channel = screen.getByRole('list', { name: 'Leute in gang' });
    expect(within(channel).getByText('ich (du)')).toBeInTheDocument();
    expect(within(channel).getByRole('button', { name: 'Ben' })).toBeInTheDocument();
  });

  it('leaves the room and is back at the join card with the invite remembered', async () => {
    render(App);
    await join();
    await fireEvent.click(within(dock()).getByRole('button', { name: 'Verlassen' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Beitreten' })).toBeInTheDocument();
    expect(screen.getByLabelText('Einladung')).toHaveValue(invite);
    expect(within(dock()).queryByRole('button', { name: 'Verlassen' })).not.toBeInTheDocument();
  });

  it('toggles mute in the dock', async () => {
    render(App);
    const mute = within(dock()).getByRole('button', { name: 'Mikrofon stummschalten' });
    expect(mute).toHaveAttribute('aria-pressed', 'false');
    await fireEvent.click(mute);
    expect(mute).toHaveAttribute('aria-pressed', 'true');
  });

  it('mutes one person for me only, on the tile', async () => {
    render(App);
    await join();
    const anna = within(stage()).getAllByRole('listitem')[0];
    await fireEvent.click(within(anna).getByRole('button', { name: 'Hörbar' }));
    expect(within(anna).getByRole('button', { name: 'Stumm für mich' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
  });

  it('opens a person in the inspector from the sidebar and closes it with Esc', async () => {
    render(App);
    await join();
    await fireEvent.click(screen.getByRole('button', { name: 'Ben' }));
    const details = screen.getByRole('complementary', { name: 'Details' });
    expect(within(details).getByRole('heading', { name: 'Ben' })).toBeInTheDocument();
    await fireEvent.click(within(details).getByRole('button', { name: 'Für mich stummschalten' }));
    expect(within(details).getByRole('button', { name: 'Für mich stumm' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.queryByRole('complementary', { name: 'Details' })).not.toBeInTheDocument();
    expect(screen.getByRole('heading', { level: 1, name: 'gang' })).toBeInTheDocument();
  });
});

describe('Ich module', () => {
  it('has the devices with a live microphone test', async () => {
    render(App);
    await openMe('Audio und Sprechen');
    expect(await screen.findByRole('option', { name: 'Webcam' })).toBeInTheDocument();
    expect(screen.getByRole('meter', { name: 'Mikrofontest' })).toBeInTheDocument();
  });

  it('shows the diagnosis with the timeline and exports it', async () => {
    render(App);
    await join();
    await openMe('Diagnose');
    const panel = screen.getByRole('region', { name: 'Diagnose' });
    expect(within(panel).getByText('verbunden')).toBeInTheDocument();
    expect(within(panel).getByText('gang')).toBeInTheDocument();
    expect(within(panel).getByText(/joining wss:\/\/voice\.example/)).toBeInTheDocument();
    await fireEvent.click(within(panel).getByRole('button', { name: 'Diagnose-Datei speichern' }));
    expect(await within(panel).findByText(/Gespeichert:/)).toBeInTheDocument();
  });

  it('shows the help and goes back to the room on Esc', async () => {
    render(App);
    await join();
    await openMe('Hilfe');
    expect(screen.getByRole('heading', { level: 1, name: 'Mittesten' })).toBeInTheDocument();
    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.getByRole('heading', { level: 1, name: 'gang' })).toBeInTheDocument();
  });

  it('explains a diagnosis value with a tooltip that leads into the glossary', async () => {
    render(App);
    await join();
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
