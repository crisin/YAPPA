<script lang="ts">
  // The window: one shell, the modules in it (ADR-004). Here the parts are wired — engine
  // session, shell, API door — and what concerns the whole app is decided: banners from
  // connection and server state, the theme, and the two keys that work everywhere
  // (`Strg K` for the palette, `Esc` to step back).
  import { onMount, untrack } from 'svelte';
  import { api as defaultApi, type Api } from './lib/api';
  import { AppState, setApp } from './lib/app.svelte';
  import { engine as defaultEngine, type Engine } from './lib/engine';
  import { help } from './lib/helpState.svelte';
  import { adminModule } from './lib/modules/admin';
  import { meModule } from './lib/modules/me';
  import { voiceModule } from './lib/modules/voice';
  import { Session } from './lib/session.svelte';
  import Onboarding from './lib/Onboarding.svelte';
  import Shell from './lib/shell/Shell.svelte';
  import { BANNER, Shell as ShellState } from './lib/shell.svelte';
  import { isBlocked } from './lib/version';

  // Tests hand in their own mocks (a scene per test); the app uses the real doors.
  let { api = defaultApi, engine = defaultEngine }: { api?: Api; engine?: Engine } = $props();
  // svelte-ignore state_referenced_locally
  const app = setApp(new AppState(new Session(engine), new ShellState(), api));
  const { session, shell, gang } = app;
  shell.register(voiceModule(app));
  shell.register(adminModule(app));
  shell.register(meModule(app));

  /** Hourly (ADR-006): the server's config — minimum version, kill switch — and a look at
   * the update channel. */
  const CONFIG_INTERVAL_MS = 60 * 60 * 1000;

  async function loadConfig() {
    try {
      app.config = await api.config();
      shell.clearBanner('offline');
      void app.updater.check();
    } catch (reason) {
      // Not logged in yet means: no server known yet — that is not "offline".
      const failure = reason as { code?: string; message?: string } | undefined;
      if (failure?.code !== 'unreachable') return;
      // The message says why (name, IPv6-only server, TLS …) — keep it, add the reassurance.
      const why = failure.message?.replace(/\.?$/, '.') ?? 'Server nicht erreichbar.';
      shell.showBanner({
        id: 'offline',
        priority: BANNER.offline,
        text: `${why} Ein laufender Call geht trotzdem weiter.`,
        action: { label: 'Erneut versuchen', run: () => void loadConfig() },
        dismissible: true,
      });
    }
  }

  onMount(() => {
    let stop: (() => void) | undefined;
    let stopLinks: (() => void) | undefined;
    let gone = false;
    void (async () => {
      app.info = await engine.info();
      void app.loadAppSettings().catch(() => {});
      if (app.info.debugPanel) app.openMe('diagnose');
      stop = await session.start();
      if (gone) stop();
      await gang.start();
      void loadConfig();
      stopLinks = await api.inviteLinks(receiveLink);
    })();
    const hourly = setInterval(() => void loadConfig(), CONFIG_INTERVAL_MS);
    return () => {
      gone = true;
      clearInterval(hourly);
      stop?.();
      stopLinks?.();
      gang.stop();
    };
  });

  $effect(() => {
    document.documentElement.dataset.theme = app.theme;
  });

  /** A clicked invite link: into the login card, or a note when already logged in. */
  function receiveLink(link: string) {
    shell.open('voice');
    if (gang.status === 'in') {
      shell.showBanner({
        id: 'invite',
        priority: BANNER.update,
        text: 'Du bist auf diesem Rechner schon angemeldet – die Einladung wird nicht gebraucht.',
        dismissible: true,
      });
    } else {
      session.invite = link;
    }
  }

  // The rail shows the admin module only to admins; a role change follows at once.
  $effect(() => {
    const role = gang.me?.role ?? 'member';
    untrack(() => shell.setRole(role));
  });

  // After a login the server is known: its config (minimum version) can be read.
  $effect(() => {
    if (gang.status === 'in') untrack(() => void loadConfig());
  });

  // A glossary link ("Mehr im Glossar") asks for the help; it lives in „Ich“.
  $effect(() => {
    if (help.open)
      untrack(() => {
        help.open = false;
        app.openMe('hilfe');
      });
  });

  // Banner slot: one message for the whole app, the most important one wins.
  $effect(() => {
    const message = session.error;
    untrack(() => {
      if (message) {
        shell.showBanner({
          id: 'error',
          priority: BANNER.error,
          text: message,
          dismissible: true,
          onDismiss: () => (session.error = null),
        });
      } else shell.clearBanner('error');
    });
  });

  // A device that is away (A2): the engine runs on the default until it is back — the
  // banner says so and goes by itself, it cannot be closed before.
  $effect(() => {
    const note = session.deviceNote;
    untrack(() => {
      if (note) {
        shell.showBanner({
          id: 'device',
          priority: BANNER.reconnecting,
          text: note,
          dismissible: false,
        });
      } else shell.clearBanner('device');
    });
  });

  // A problem while switching channels during a call (token refused, server away): the
  // stage shows the people then, so the banner carries it; outside a room the stage does.
  $effect(() => {
    const problem = session.inRoom ? gang.problem : null;
    untrack(() => {
      if (problem) {
        shell.showBanner({
          id: 'gang',
          priority: BANNER.error,
          text: problem,
          dismissible: true,
          onDismiss: () => (gang.problem = null),
        });
      } else shell.clearBanner('gang');
    });
  });

  $effect(() => {
    const lost = session.connection === 'reconnecting';
    untrack(() => {
      if (lost) {
        shell.showBanner({
          id: 'link',
          priority: BANNER.reconnecting,
          text: 'Verbindung verloren – yAPPA verbindet neu. Du musst nichts tun.',
          dismissible: false,
        });
      } else shell.clearBanner('link');
    });
  });

  // A downloaded update (D1): a polite banner until the person clicks; in a call the click
  // leaves it first. Closed, it comes back with the next hourly check.
  $effect(() => {
    const ready = app.updater.ready;
    const inRoom = session.inRoom;
    untrack(() => {
      if (ready) {
        shell.showBanner({
          id: 'update',
          priority: BANNER.update,
          text: `Update auf ${ready.version} ist bereit.${inRoom ? ' Zum Installieren wird der Call verlassen.' : ''}`,
          action: { label: 'Jetzt installieren', run: () => void app.installUpdate() },
          dismissible: true,
        });
      } else shell.clearBanner('update');
    });
  });

  // Too old or killed (ADR-006): out of the call, and the banner stays until the update
  // is installed — the server refuses this version a token anyway.
  $effect(() => {
    const config = app.config;
    const version = app.info?.version;
    const blocked =
      config && version && isBlocked(version, config.minVersion, config.killedVersions);
    const ready = app.updater.ready;
    const inRoom = session.inRoom;
    untrack(() => {
      if (config && version && blocked) {
        if (inRoom) void app.leaveVoice();
        shell.showBanner({
          id: 'blocked',
          priority: BANNER.blocked,
          text: `Diese Version (${version}) ist zu alt für den Server – mindestens ${config.minVersion} wird gebraucht.${ready ? '' : ' Bitte aktualisieren.'}`,
          action: ready
            ? { label: 'Jetzt installieren', run: () => void app.installUpdate() }
            : { label: 'Nach Update suchen', run: () => void app.updater.check() },
          dismissible: false,
        });
      } else shell.clearBanner('blocked');
    });
  });

  function onKey(event: KeyboardEvent) {
    if (event.defaultPrevented) return;
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
      event.preventDefault();
      shell.togglePalette();
    } else if (event.key === 'Escape' && !shell.palette.open) {
      if (shell.inspector.open) shell.closeInspector();
      else shell.back();
    }
  }
</script>

<svelte:window onkeydown={onKey} />

{#if app.showOnboarding}
  <Onboarding />
{:else}
  <Shell />
{/if}
