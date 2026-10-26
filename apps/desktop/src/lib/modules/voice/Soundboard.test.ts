// The soundboard inspector (ADR-010) against the mock engine: the folder as buttons, a click
// plays at the sound's own volume, the playing state shows, stop stops, a wrong file is
// refused in German, „Bearbeiten“ sets a volume per sound and removes one, and a person
// playing a sound is said so in the channel.
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import App from '../../../App.svelte';
import { createMockApi } from '../../mockApi';
import { MOCK_SOUNDS, createMockEngine } from '../../mockEngine';
import type { Command } from '../../types/Command';
import { axe } from '../../../test-a11y';

/** The app in the call scene, joined into the raid, with the soundboard open; records what
 * the UI sent to the engine. */
async function openSoundboard() {
  const mock = createMockEngine('call');
  const sent: Command[] = [];
  const engine = {
    ...mock,
    send(command: Command) {
      sent.push(command);
      return mock.send(command);
    },
  };
  const api = createMockApi('call', engine);
  const result = render(App, { props: { api, engine } });
  await screen.findByRole('heading', { level: 1, name: /^Hallo / });
  const channels = screen.getByRole('list', { name: 'Voice-Channels' });
  await fireEvent.click(within(channels).getByRole('button', { name: /^Raid-Abend/ }));
  await screen.findByRole('heading', { level: 1, name: 'Raid-Abend' });
  await fireEvent.click(screen.getByRole('button', { name: /^Soundboard/ }));
  const details = screen.getByRole('complementary', { name: 'Details' });
  await within(details).findByRole('list', { name: 'Sounds' });
  return { ...result, details, sent, engine };
}

describe('Soundboard', () => {
  it('lists the folder as buttons and plays a sound on click', async () => {
    const { details, sent, container } = await openSoundboard();
    const pads = within(within(details).getByRole('list', { name: 'Sounds' })).getAllByRole(
      'button',
    );
    expect(pads.map((b) => b.textContent?.trim())).toEqual(MOCK_SOUNDS.map((s) => s.name));
    expect(within(details).getByRole('button', { name: 'Stopp' })).toBeDisabled();

    await fireEvent.click(pads[0]);
    expect(sent).toContainEqual({ type: 'playSound', path: MOCK_SOUNDS[0].path, gainDb: 0 });
    // Tusch has its own volume in the folder: it travels with the click.
    await fireEvent.click(pads[1]);
    expect(sent).toContainEqual({ type: 'playSound', path: MOCK_SOUNDS[1].path, gainDb: -6 });
    await waitFor(() => expect(pads[0]).toHaveAttribute('aria-pressed', 'true'));
    expect(screen.getByRole('button', { name: 'Soundboard · spielt' })).toBeInTheDocument();
    expect(within(details).getByRole('button', { name: 'Stopp' })).toBeEnabled();
    expect(await axe(container)).toHaveNoViolations();

    await fireEvent.click(within(details).getByRole('button', { name: 'Stopp' }));
    expect(sent).toContainEqual({ type: 'stopSounds' });
    await waitFor(() => expect(pads[0]).toHaveAttribute('aria-pressed', 'false'));
  });

  it('filters by name and sets my soundboard volume in the settings', async () => {
    const { details, sent } = await openSoundboard();
    await fireEvent.input(within(details).getByRole('searchbox', { name: 'Sounds suchen' }), {
      target: { value: 'zo' },
    });
    const pads = within(within(details).getByRole('list', { name: 'Sounds' })).getAllByRole(
      'button',
    );
    expect(pads.map((b) => b.textContent?.trim())).toEqual(['Zonk']);

    await fireEvent.input(within(details).getByLabelText('Lautstärke meiner Sounds'), {
      target: { value: '-12' },
    });
    const applied = [...sent].reverse().find((c) => c.type === 'applySettings');
    expect(applied?.type === 'applySettings' && applied.settings.soundboardGainDb).toBe(-12);
  });

  it('adds a picked sound file to the folder and refuses a text file in German', async () => {
    const { details } = await openSoundboard();
    const picker = within(details).getByLabelText('Sounddatei wählen') as HTMLInputElement;
    const sound = new File([new Uint8Array([1, 2, 3])], 'Fanfare.mp3', { type: 'audio/mpeg' });
    Object.defineProperty(picker, 'files', { value: [sound], configurable: true });
    await fireEvent.change(picker);
    const list = within(details).getByRole('list', { name: 'Sounds' });
    await within(list).findByRole('button', { name: 'Fanfare' });

    const text = new File(['hallo'], 'notizen.txt', { type: 'text/plain' });
    Object.defineProperty(picker, 'files', { value: [text], configurable: true });
    await fireEvent.change(picker);
    expect(await within(details).findByRole('alert')).toHaveTextContent(/keine Sounddatei/);
  });

  it('edits a volume per sound (live and remembered) and moves a sound to the bin', async () => {
    const { details, sent, engine } = await openSoundboard();
    const remember = vi.spyOn(engine, 'setSoundGain');
    await fireEvent.click(within(details).getByRole('button', { name: 'Bearbeiten' }));
    const rows = within(details).getByRole('list', { name: 'Sounds bearbeiten' });
    const slider = within(rows).getByLabelText('Lautstärke Airhorn');
    expect(slider).toHaveValue('0');
    // While the slider moves, the engine follows and nothing is written yet.
    await fireEvent.input(slider, { target: { value: '-9' } });
    expect(sent).toContainEqual({ type: 'setSoundGain', path: MOCK_SOUNDS[0].path, gainDb: -9 });
    expect(remember).not.toHaveBeenCalled();
    expect(within(rows).getByText('-9 dB')).toBeInTheDocument();
    // Letting go writes it down, and the next click plays at that volume.
    await fireEvent.change(slider, { target: { value: '-9' } });
    expect(remember).toHaveBeenCalledWith(MOCK_SOUNDS[0].path, -9);
    await fireEvent.click(within(details).getByRole('button', { name: 'Fertig' }));
    const pads = within(within(details).getByRole('list', { name: 'Sounds' })).getAllByRole(
      'button',
    );
    await fireEvent.click(pads[0]);
    expect(sent).toContainEqual({ type: 'playSound', path: MOCK_SOUNDS[0].path, gainDb: -9 });

    await fireEvent.click(within(details).getByRole('button', { name: 'Bearbeiten' }));
    await fireEvent.click(within(details).getByRole('button', { name: 'Zonk löschen' }));
    await waitFor(() =>
      expect(within(details).queryByLabelText('Lautstärke Zonk')).not.toBeInTheDocument(),
    );
    expect(within(details).getByLabelText('Lautstärke Airhorn')).toBeInTheDocument();
    vi.restoreAllMocks();
  });

  it('says in the channel when somebody plays a sound', async () => {
    await openSoundboard();
    // The mock's second person in the raid (Sasha) clicks their soundboard for a second out
    // of every three: the card, the sidebar and the inspector say so while it lasts.
    const people = screen.getByRole('list', { name: 'Leute in Raid-Abend' });
    await within(people).findByRole('img', { name: 'spielt einen Sound' }, { timeout: 4000 });
    expect(screen.getByText('spielt einen Sound')).toBeInTheDocument();
    await fireEvent.click(within(people).getByRole('button', { name: 'Sasha' }));
    const details = screen.getByRole('complementary', { name: 'Details' });
    expect(within(details).getByText('spielt einen Sound')).toBeInTheDocument();
  });

  it('gives a person a soundboard volume of their own in the inspector', async () => {
    const { sent } = await openSoundboard();
    const raid = screen.getByRole('list', { name: 'Leute in Raid-Abend' });
    await fireEvent.click(within(raid).getByRole('button', { name: 'Ben' }));
    const details = screen.getByRole('complementary', { name: 'Details' });
    await fireEvent.click(
      within(details).getByRole('button', { name: 'Soundboard für mich stummschalten' }),
    );
    expect(sent).toContainEqual({
      type: 'setPeerMuted',
      identity: 'ben',
      track: 'soundboard',
      muted: true,
    });
    expect(
      within(details).getByRole('button', { name: 'Soundboard für mich stumm' }),
    ).toHaveAttribute('aria-pressed', 'true');
    // The voice mute is untouched.
    expect(within(details).getByRole('button', { name: 'Für mich stummschalten' })).toHaveAttribute(
      'aria-pressed',
      'false',
    );
    vi.restoreAllMocks();
  });
});
