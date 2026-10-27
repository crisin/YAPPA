// The app audio inspector (ADR-011) against the mock engine: the button shows only where
// the OS can capture, the list comes from the engine, a click streams one application and
// the state follows (card button, inspector, sidebar), stop ends it, the slider sets my gain,
// a person streaming is said so in the channel, and a person gets an app audio volume and
// mute of their own.
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import App from '../../../App.svelte';
import { createMockApi } from '../../mockApi';
import { MOCK_APPS, createMockEngine } from '../../mockEngine';
import type { Command } from '../../types/Command';
import type { Event } from '../../types/Event';
import { axe } from '../../../test-a11y';

/** The app in the call scene, joined into the raid; records what the UI sent. */
async function joinRaid(engine = createMockEngine('call')) {
  const sent: Command[] = [];
  const recording = {
    ...engine,
    send(command: Command) {
      sent.push(command);
      return engine.send(command);
    },
  };
  const api = createMockApi('call', recording);
  const result = render(App, { props: { api, engine: recording } });
  await screen.findByRole('heading', { level: 1, name: /^Hallo / });
  const channels = screen.getByRole('list', { name: 'Voice-Channels' });
  await fireEvent.click(within(channels).getByRole('button', { name: /^Raid-Abend/ }));
  await screen.findByRole('heading', { level: 1, name: 'Raid-Abend' });
  return { ...result, sent };
}

async function openAppAudio() {
  const result = await joinRaid();
  await fireEvent.click(await screen.findByRole('button', { name: /^App-Audio/ }));
  const details = screen.getByRole('complementary', { name: 'Details' });
  await within(details).findByRole('list', { name: 'Programme mit Ton' });
  return { ...result, details };
}

describe('App-Audio', () => {
  it('hides the button where the OS cannot capture an application', async () => {
    const mock = createMockEngine('call');
    // An engine on macOS: the apps event says "not available".
    const engine = {
      ...mock,
      subscribe(handler: (event: Event) => void) {
        return mock.subscribe((event) =>
          handler(event.type === 'apps' ? { type: 'apps', available: false, apps: [] } : event),
        );
      },
    };
    await joinRaid(engine);
    expect(screen.getByRole('button', { name: /^Soundboard/ })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^App-Audio/ })).not.toBeInTheDocument();
  });

  it('lists the applications, streams one on click and stops it', async () => {
    const { details, sent, container } = await openAppAudio();
    expect(sent).toContainEqual({ type: 'listApps' });
    const apps = within(
      within(details).getByRole('list', { name: 'Programme mit Ton' }),
    ).getAllByRole('button');
    expect(apps.map((b) => b.textContent?.replace(/\s+/g, ' ').trim())).toEqual([
      'eldenring spielt gerade',
      'firefox still',
    ]);
    expect(within(details).queryByRole('button', { name: 'Stopp' })).not.toBeInTheDocument();

    await fireEvent.click(apps[0]);
    expect(sent).toContainEqual({
      type: 'startAppAudio',
      pid: MOCK_APPS[0].pid,
      name: 'eldenring',
    });
    await waitFor(() => expect(apps[0]).toHaveAttribute('aria-pressed', 'true'));
    expect(within(details).getByRole('status')).toHaveTextContent('Streamt eldenring');
    expect(screen.getByRole('button', { name: 'App-Audio · streamt' })).toBeInTheDocument();
    // My own line in the channel list carries the marker.
    const people = screen.getByRole('list', { name: 'Leute in Raid-Abend' });
    expect(
      within(people).getAllByRole('img', { name: 'streamt App-Audio' }).length,
    ).toBeGreaterThan(0);
    expect(await axe(container)).toHaveNoViolations();

    await fireEvent.click(within(details).getByRole('button', { name: 'Stopp' }));
    expect(sent).toContainEqual({ type: 'stopAppAudio' });
    await waitFor(() => expect(apps[0]).toHaveAttribute('aria-pressed', 'false'));
    expect(screen.getByRole('button', { name: 'App-Audio' })).toBeInTheDocument();
  });

  it('reads the list again and sets my app audio volume in the settings', async () => {
    const { details, sent } = await openAppAudio();
    const before = sent.filter((c) => c.type === 'listApps').length;
    await fireEvent.click(within(details).getByRole('button', { name: 'Neu einlesen' }));
    expect(sent.filter((c) => c.type === 'listApps').length).toBe(before + 1);

    await fireEvent.input(within(details).getByLabelText('Lautstärke meines App-Audios'), {
      target: { value: '-9' },
    });
    const applied = [...sent].reverse().find((c) => c.type === 'applySettings');
    expect(applied?.type === 'applySettings' && applied.settings.appAudioGainDb).toBe(-9);
    expect(within(details).getByText('-9 dB')).toBeInTheDocument();
  });

  it('says in the channel when somebody streams, and gives them a volume of their own', async () => {
    const { sent } = await joinRaid();
    // The mock's third person in the raid (Tim) streams a game for two seconds out of
    // every four: the card and the sidebar say so while it lasts.
    const people = screen.getByRole('list', { name: 'Leute in Raid-Abend' });
    await within(people).findByRole('img', { name: 'streamt App-Audio' }, { timeout: 5000 });
    expect(screen.getByText('streamt')).toBeInTheDocument();

    await fireEvent.click(within(people).getByRole('button', { name: 'Tim' }));
    const details = screen.getByRole('complementary', { name: 'Details' });
    await fireEvent.input(within(details).getByLabelText('App-Audio von Tim für mich'), {
      target: { value: '-12' },
    });
    expect(sent).toContainEqual({
      type: 'setPeerVolume',
      identity: 'tim',
      track: 'music',
      gainDb: -12,
    });
    await fireEvent.click(
      within(details).getByRole('button', { name: 'App-Audio für mich stummschalten' }),
    );
    expect(sent).toContainEqual({
      type: 'setPeerMuted',
      identity: 'tim',
      track: 'music',
      muted: true,
    });
    expect(
      within(details).getByRole('button', { name: 'App-Audio für mich stumm' }),
    ).toHaveAttribute('aria-pressed', 'true');
    // Voice and soundboard mutes are untouched.
    expect(within(details).getByRole('button', { name: 'Für mich stummschalten' })).toHaveAttribute(
      'aria-pressed',
      'false',
    );
    expect(
      within(details).getByRole('button', { name: 'Soundboard für mich stummschalten' }),
    ).toHaveAttribute('aria-pressed', 'false');
  });
});
