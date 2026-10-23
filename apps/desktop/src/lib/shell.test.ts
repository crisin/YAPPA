import type { Component } from 'svelte';
import { describe, expect, it } from 'vitest';
import { BANNER, Shell, type ModuleDef } from './shell.svelte';

// The shell never renders in these tests; any object does as a component.
const dummy = {} as Component;

const module = (id: string, extra: Partial<ModuleDef> = {}): ModuleDef => ({
  id,
  label: id[0].toUpperCase() + id.slice(1),
  icon: dummy,
  sidebar: dummy,
  stage: dummy,
  ...extra,
});

function shell(role: 'member' | 'admin' = 'member') {
  const s = new Shell();
  s.role = role;
  s.register(
    module('voice', {
      commands: () => [{ id: 'join:lobby', label: 'Lobby', group: 'Channels', run() {} }],
    }),
  );
  s.register(module('me'));
  s.register(module('admin', { role: 'admin' }));
  return s;
}

describe('Shell registry', () => {
  it('splits the rail into the module list and the bottom slot', () => {
    const s = new Shell();
    s.register(module('voice'));
    s.register(module('me', { slot: 'bottom' }));
    expect(s.railMain.map((m) => m.id)).toEqual(['voice']);
    expect(s.railBottom.map((m) => m.id)).toEqual(['me']);
  });

  it('shows modules by role and starts on the first visible one', () => {
    const member = shell();
    expect(member.visible.map((m) => m.id)).toEqual(['voice', 'me']);
    expect(member.active).toBe('voice');
    expect(shell('admin').visible.map((m) => m.id)).toEqual(['voice', 'me', 'admin']);
  });

  it('replaces a module registered twice instead of duplicating it', () => {
    const s = shell();
    s.register(module('me', { label: 'Ich' }));
    expect(s.modules.filter((m) => m.id === 'me')).toHaveLength(1);
    expect(s.activeModule?.id).toBe('voice');
    s.open('me');
    expect(s.activeModule?.label).toBe('Ich');
  });

  it('ignores opening a module the role may not see', () => {
    const s = shell();
    s.open('admin');
    expect(s.active).toBe('voice');
  });

  it('falls back to a visible module when the role shrinks', () => {
    const s = shell('admin');
    s.open('admin');
    s.setRole('member');
    expect(s.active).toBe('voice');
  });
});

describe('Shell navigation', () => {
  it('goes back to the previous module on Esc and to the first one at the end', () => {
    const s = shell('admin');
    s.open('me');
    s.open('admin');
    s.back();
    expect(s.active).toBe('me');
    s.back();
    expect(s.active).toBe('voice');
    s.back();
    expect(s.active).toBe('voice');
  });

  it('remembers per module whether the inspector was open', () => {
    const s = shell();
    s.openInspector({ kind: 'person', id: 'ben' });
    s.open('me');
    expect(s.inspector.open).toBe(false);
    s.open('voice');
    expect(s.inspector.open).toBe(true);
    // …but not what it showed: the selection is the module's business.
    expect(s.inspector.content).toBeNull();
  });
});

describe('Shell banner slot', () => {
  it('shows the highest priority and replaces a banner with the same id', () => {
    const s = shell();
    s.showBanner({
      id: 'update',
      priority: BANNER.update,
      text: 'Update bereit',
      dismissible: true,
    });
    s.showBanner({
      id: 'link',
      priority: BANNER.reconnecting,
      text: 'Verbinde neu',
      dismissible: false,
    });
    expect(s.banner?.id).toBe('link');
    s.showBanner({ id: 'link', priority: BANNER.offline, text: 'Offline', dismissible: false });
    expect(s.banners).toHaveLength(2);
    expect(s.banner?.text).toBe('Offline');
  });

  it('tells the cause when a banner is dismissed', () => {
    const s = shell();
    let forgotten = 0;
    s.showBanner({
      id: 'error',
      priority: BANNER.error,
      text: 'Kaputt',
      dismissible: true,
      onDismiss: () => forgotten++,
    });
    s.dismissBanner('error');
    s.dismissBanner('error');
    expect(forgotten).toBe(1);
  });

  it('lets people dismiss only dismissible banners', () => {
    const s = shell();
    s.showBanner({
      id: 'blocked',
      priority: BANNER.blocked,
      text: 'Version gesperrt',
      dismissible: false,
    });
    s.showBanner({
      id: 'update',
      priority: BANNER.update,
      text: 'Update bereit',
      dismissible: true,
    });
    s.dismissBanner('blocked');
    s.dismissBanner('update');
    expect(s.banners.map((b) => b.id)).toEqual(['blocked']);
    s.clearBanner('blocked');
    expect(s.banner).toBeUndefined();
  });
});

describe('Shell palette', () => {
  it('offers module switching plus every visible module’s commands', () => {
    const s = shell();
    const ids = s.commands().map((c) => c.id);
    expect(ids).toEqual(['module:voice', 'module:me', 'join:lobby']);
    expect(
      shell('admin')
        .commands()
        .map((c) => c.id),
    ).toContain('module:admin');
  });

  it('runs a switching command and toggles open state', () => {
    const s = shell();
    s.togglePalette();
    expect(s.palette.open).toBe(true);
    s.commands()
      .find((c) => c.id === 'module:me')
      ?.run();
    expect(s.active).toBe('me');
    s.togglePalette();
    expect(s.palette.open).toBe(false);
  });
});
