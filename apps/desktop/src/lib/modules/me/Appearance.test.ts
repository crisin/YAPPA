// „Ich → Darstellung“: the X of the window goes into the tray by default, the setting
// switches that, and autostart is a checkbox — all stored as app settings.
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import App from '../../../App.svelte';
import { createMockApi } from '../../mockApi';
import { createMockEngine } from '../../mockEngine';
import { axe } from '../../../test-a11y';

async function openAppearance() {
  const engine = createMockEngine('call');
  const api = createMockApi('call', engine);
  const result = render(App, { props: { api, engine } });
  await screen.findByRole('heading', { level: 1, name: /^Hallo / });
  const rail = screen.getByRole('navigation', { name: 'Bereiche' });
  await fireEvent.click(within(rail).getByRole('button', { name: 'Ich' }));
  const nav = screen.getByRole('navigation', { name: 'Einstellungen' });
  await fireEvent.click(await within(nav).findByRole('button', { name: 'Darstellung' }));
  await screen.findByRole('heading', { level: 1, name: 'Darstellung' });
  return { ...result, api };
}

const autostartBox = () =>
  screen.getByRole('checkbox', { name: /beim Anmelden am Rechner starten/ });

describe('Darstellung: Fenster und Start', () => {
  it('closes into the tray by default and stores the other choice', async () => {
    const { api, container } = await openAppearance();
    const tray = screen.getByRole('radio', { name: /^In den Infobereich/ });
    const quit = screen.getByRole('radio', { name: 'yAPPA beenden' });
    expect(tray).toBeChecked();
    expect(quit).not.toBeChecked();
    expect((await api.appSettings()).onClose).toBe('tray');
    expect(await axe(container)).toHaveNoViolations();

    await fireEvent.click(quit);
    await waitFor(async () => expect((await api.appSettings()).onClose).toBe('quit'));
    expect(quit).toBeChecked();
    await fireEvent.click(tray);
    await waitFor(async () => expect((await api.appSettings()).onClose).toBe('tray'));
  });

  it('switches autostart on and off', async () => {
    const { api } = await openAppearance();
    expect(autostartBox()).not.toBeChecked();
    await fireEvent.click(autostartBox());
    await waitFor(async () => expect((await api.appSettings()).autostart).toBe(true));
    expect(autostartBox()).toBeChecked();
    await fireEvent.click(autostartBox());
    await waitFor(async () => expect((await api.appSettings()).autostart).toBe(false));
    expect(autostartBox()).not.toBeChecked();
  });

  it('shows what the shell stored when the OS refuses autostart', async () => {
    const { api } = await openAppearance();
    const refusing = api.setAppSettings;
    api.setAppSettings = (settings) => refusing({ ...settings, autostart: false });
    await fireEvent.click(autostartBox());
    await waitFor(() => expect(autostartBox()).not.toBeChecked());
    expect((await api.appSettings()).autostart).toBe(false);
  });
});
