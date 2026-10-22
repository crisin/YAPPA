# Tasks

Variant B has no App Hub board — this file is the board. One task = one branch
(`task/<short-name>`), reviewed before it is merged into `main`. Move a line when its state
changes; finished tasks get a line in `CHANGELOG.md`.

States: **now** (in progress, max. 2) · **next** (planned, ordered) · **later** · **done**.

## Now

- [ ] S0 rest: on the MacBook `cargo xtask setup` + `check` + `build` (.app/.dmg); click
      through `cargo xtask dev` there

## Next

Phase 1 in four milestones (`docs/01-vision-v1.md`, section 9). Order is dependency.

**M1 · Shell** — nightly with the new structure, still pasted invites

- [ ] U1 Shell: zones A–G as components (rail, sidebar/stage slots, voice dock, topbar +
      `Strg K` palette, banner slot, inspector); module „Ich“ = settings navigation +
      stage + help, `Esc` returns (no overlay); the test client moves in without losing
      anything; reconnect/offline in dock and banner → first nightly with the new structure
- [ ] First evening with the gang (user): home server up (`docs/homeserver.md`), installer
      + invites out (`docs/mittesten.md`), collect diagnostics files; write down what broke
- [ ] Homeserver walkthrough (user, hands-on): follow `docs/homeserver.md` on the mini PC
      behind the FRITZ!Box — DS-Lite check first; note in the guide what did not match

**M2 · Login and channels** — invite link, name, lobby, who is where, join by click

- [ ] S6 Control plane (cut per ADR-005): axum + sqlx/SQLite in the LiveKit compose;
      `POST /auth/invite`, `GET /me|channels|config`, `POST /channels/{id}/token` (voice and
      presence tokens via `livekit-api`), `POST /livekit/webhook` → event log only; admin
      endpoints (invites, members, roles, kick via RoomService, channels, events) second;
      no presence table, no push → `docs/spikes/s6-control.md`
- [ ] C1 `crates/client` (ADR-007): API client with cache, session + presence token in
      `keyring` with file fallback, `transport::PresenceLink` (presence room, attributes
      `channel`/`app`/`talk`/`away`), `AppSettings` v1 + engine `Settings` v2 with
      migration, `YAPPA_PROFILE`; Tauri commands `api_*`; the headless example logs in the
      same way; `cargo xtask livekit presence` to watch the room
- [ ] U2 Voice module, call screen: channel tree with people, person tiles (states as
      text, `talk` from presence), join on click/Enter, person inspector (volume, local
      mute); soundboard/app-audio hidden until the engine reports them; against the mock
- [ ] U5 Bind the client: `api.ts` → Tauri; deep link `yappa://invite/<code>` +
      single instance, paste as fallback; presence → channel tree; token per channel →
      join; rewrite `docs/mittesten.md`

**M3 · Voice** — noise reduction, gate, PTT in the game, Einfach/Profi

- [ ] S1 rest: mouth-to-ear with a loopback cable (Focusrite out → in) via
      `yappa-poc measure --acoustic`; second machine in the LAN: cable pulled at the PC,
      Wi-Fi switch, speech by ear at 5/10/20 % loss; spike on the MacBook → add to
      `docs/spikes/s1-poc.md`
- [ ] E0 Engine prep (realtime review 2026-10-23, vision section 5.1): f32 capture ring,
      quantize at send; pump busy time in `Stats`; FTZ/DAZ on the pump; `crates/denoise`
      with the `Denoiser` trait; master stage with limiter in the mixer; capture channel
      choice; `AppInfo` into engine-protocol; cushion coupled to the callback size
- [ ] S2 DSP chain offline: WAV → denoiser → gate → EQ → compressor → WAV; DeepFilterNet-LL
      (git dependency, tract) vs RNNoise (`nnnoiseless`); DFN stays default only if the
      chain adds ≤ 25 ms and ≤ 5 ms CPU per 10 ms block → `docs/spikes/s2-dsp.md`
- [ ] E1 Chain in the pump: denoiser, gate/VAD with hold and hysteresis, EQ, compressor;
      `Event::StageLevels`, chain CPU and latency in `Stats`; bench before and after;
      realtime review
- [ ] S3 PTT: `GlobalHotkey` in `crates/platform-windows` (low-level hook or Raw Input, not
      the Tauri plugin — `RegisterHotKey` keeps the key from the game), engine
      `SetPttPressed`, verified in a fullscreen game; tray; autostart →
      `docs/spikes/s3-ptt.md`
- [ ] U3 Settings Einfach/Profi in module „Ich“: `AppSettings` (view, theme, hotkeys,
      notifications, update channel) and engine settings v2; signal chain as stages with
      live levels; diagnosis = the debug panel; the "Audio und Sprechen" component is built
      for reuse in onboarding

**M4 · Operations** — auto-update, admin, onboarding, banners

- [ ] D1 Release pipeline (ADR-006): `cargo xtask release <channel>` (version, changelog
      cut, build, sign, `latest.json`, upload to the box), `tauri-plugin-updater` +
      `tauri-plugin-process`, Caddy `/updates`, `GET /config` min version + kill switch →
      banner slot and forced leave; update channel in Profi
- [ ] U4 Onboarding: name → microphone (component from U3) → speaking mode (speakers →
      PTT recommended, no AEC in v1) → lobby
- [ ] U6 Admin module: people (members with client version and presence, invites, roles,
      kick), structure (channels), operations (event log, min version, kill switch); what
      the control plane cannot do yet is hidden
- [ ] Lofi round 2 (user): draw reconnect/offline/update banner and lobby radio against the
      running shell (detail questions follow established patterns, vision section 12)
- [ ] Infra for three stages (vision section 12): one compose set for LiveKit + control +
      Caddy, `home.yml` (mini PC) and new `vps.yml` differ only in `.env`/overrides; VPS is
      the update host; `docs/homeserver.md` gets a VPS part
- [ ] Optics round (user, after U3): 2–3 token variants on the running shell, then ADR

## Later

- [ ] Engine follow-ups (ADR-003, realtime reviews 2026-10-10 and 2026-10-23): drift
      compensation = hold the playout ring at the cushion; resampling for devices that are
      not at 48 kHz; transport events carry a session number; a "nothing arrives" warning
      before the SDK notices (≈ 14 s); `RING` vs `MAX_CALLBACK_FRAMES` cap; RMS into `dsp`;
      per-sample `ring.push` → chunked writes in the capture callback
- [ ] Echo cancellation for people on speakers: the Rust SDK (0.9.6) exposes no audio
      processing module, so this is an own AEC — phase 2; v1 is headset + PTT hint
- [ ] Outage detection: the client needs ≈ 14 s to notice a dead link (S1) — find the
      source (signal ping timeout?) and detect silence on the wire sooner
- [ ] `lk load-test` with 20 publishers against the dev server (needs the `lk` CLI)
- [ ] Bumps when the files are touched anyway: LiveKit server v1.13.10, `livekit` 0.9.6
- [ ] Phase 2 · Quality: second media server with failover (presence room needs a home,
      ADR-005 revisit), Prometheus/Grafana, load, AEC, four weeks without an outage
- [ ] Phase 3 · Extensions: soundboard (inspector, byte streams on the presence room,
      `soundboard` track), app audio (`AppCapture`, music track, stage tile), lobby radio,
      effects, S4/CLAP, text chat (`chat` topic), hub module
- [ ] Mobile: sketch in `docs/01-vision-v1.md` section 10; ADR (way A or B) before the first
      task
- [ ] Session-Radio (Phase 3, idea 2026-10-23, vision section 11): the lobby radio plays
      songs a local AI pipeline made from the evening's topics. *Problem while gaming:* the
      lobby is silent and generic; the running jokes of one evening become the soundtrack of
      the next. *Planes:* client only — the engine gets a session tap (output mix + own
      voice as a non-RT sink), `crates/radio` does local ASR + topics, a local sidecar makes
      the songs after the session; no control plane, no media plane (shared radio later =
      music track). *Not built:* cloud services, server-side processing, raw recordings kept
      beyond the session, recording without a REC state visible to the channel. Spike S7
      (topic quality, GPU time per song, disk, which sidecar) and an ADR first.

## Open questions

- Logo: the cat (2026-10-07) is a "for now" logo of unknown origin, and the repo is public
  (`crisin/YAPPA`) — clarify the rights or replace it.
- License: MIT/Apache (no VST3) — same open decision as variant A. Cargo metadata says
  `MIT OR Apache-2.0` provisionally.

## Done

- [x] U0 UI foundation (ADR-004): `bits-ui` 2.20, `@lucide/svelte`, `vitest-axe` with an
      axe test on the app; `ui-tokens` v1 (rail/inspector widths, type scale, state colors,
      focus, overlay, motion); second door `api.ts` + `mockApi.ts` with the "Die Gang"
      fixture and admin actions; mock scenes `?scene=` / `cargo xtask dev --ui --scene`
      for engine and API; `shell.svelte.ts` (module registry by role, back navigation,
      inspector memory, banner priorities, palette commands) with tests; api-types gained
      `Me`, `PublicConfig`, `Member`, `InviteCode`, `EventEntry`, `PresenceState` +
      attribute mapping (ADR-005), unused `Presence` removed; `gen-types` cleans stale files
      (2026-10-23)
- [x] Architecture review v1: `docs/01-vision-v1.md`, ADR-004 to 007 accepted, board in
      milestones M1–M4; repo on GitHub (`crisin/YAPPA`, public, default branch `main`)
      (2026-10-23)
- [x] In-app docs: `cargo xtask docs` renders docs/, TASKS.md and CHANGELOG.md into the
      app ("Hilfe" view with Hilfe/Projekt groups, search, in-app links); user help
      `docs/hilfe/` (how it works, glossary); glossary entries as ⓘ tooltips in the debug
      panel and on labels; web links via `open_url` (2026-10-23)
- [x] Test client: `crates/transport` (trait + LiveKit), `crates/audio-engine` (devices,
      pump, 24-slot mixer, levels, rejoin), shell with logging and diagnostics export, UI
      with invite join, participants, per-person volume, debug panel; headless example;
      ADR-003; tester guide `docs/mittesten.md` (2026-10-10)
- [x] Home server (stage 1) prepared: `infra/livekit/home.yml` (LiveKit + Caddy, host
      network), `.env.example`, `cargo xtask livekit token <name>`, `yappa-poc --token`,
      guide `docs/homeserver.md` for FRITZ!Box + Linux VM (2026-10-10)
- [x] S1: PoC — `crates/spike-s1` (`yappa-poc`): two clients hear each other via local
      LiveKit, RED/bitrate verified, click latency 50–60 ms (digital path), loss and
      reconnect measured → `docs/spikes/s1-poc.md`; Rust SDK stays (2026-10-10)
- [x] S1 prep: `infra/livekit/dev.yml` (LiveKit v1.13.8 + netem sidecar) and
      `cargo xtask livekit up|down|logs|loss|cut` (2026-10-10)
- [x] Logo "for now": cat as app icon set, sidebar logo and favicon; `cargo xtask dev`
      checked on Windows by the user (2026-10-07)
- [x] Dev/prod build commands: `cargo xtask dev|build`, npm `app:*` scripts, release profile,
      bundle config, READMEs (2026-10-07)
- [x] Dev setup: pinned toolchain, `cargo xtask setup|doctor|test|bench|coverage|hook`,
      nextest, cargo-deny, insta + proptest + criterion, vitest + Testing Library + prettier,
      git hooks, Claude Code settings/hooks/skills/agent, `docs/dev-setup.md` (2026-10-07)
- [x] S0: CI workflow — windows + macos matrix running `cargo xtask check --ci` (dormant
      until a remote exists) (2026-10-07)
- [x] S0: `apps/desktop` — Tauri 2 + Svelte 5 + Vite + TS shell, engine stub, ui-tokens →
      tokens.css, one-window UI (2026-10-07)
- [x] S0: Cargo workspace, `cargo xtask check|gen-types|tokens`, `engine-protocol`,
      `api-types`, `dsp` (biquad) (2026-10-07)
- [x] ADR-002: alternatives to Rust checked — Rust stays (2026-10-07)
- [x] Repository created: git, planning baseline, ADR-001 (variant B stack), docs skeleton
      (2026-10-07)
