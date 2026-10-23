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

- [ ] First evening with the gang (user): home server up (`docs/homeserver.md`), installer
      + invites out (`docs/mittesten.md`), collect diagnostics files; write down what broke
- [ ] Homeserver walkthrough (user, hands-on): follow `docs/homeserver.md` on the mini PC
      behind the FRITZ!Box — DS-Lite check first; note in the guide what did not match

**M2 · Login and channels** — invite link, name, lobby, who is where, join by click

**M3 · Voice** — noise reduction, gate, PTT in the game, Einfach/Profi

- [ ] S1 rest: mouth-to-ear with a loopback cable (Focusrite out → in) via
      `yappa-poc measure --acoustic`; second machine in the LAN: cable pulled at the PC,
      Wi-Fi switch, speech by ear at 5/10/20 % loss; spike on the MacBook → add to
      `docs/spikes/s1-poc.md`
- [ ] Listen to S2 (user): `target/s2/{off,rnnoise,deepfilter}.wav` (rerun
      `cargo run --release -p spike-s2 -- target/s2/noisy_snr0.wav` after a fresh clone);
      record a real gaming-evening noise and run it through
- [ ] PTT in a fullscreen game (user, testabend): does the voice come with the key, does
      the game still get the key? Note anti-cheat games separately → `docs/spikes/s3-ptt.md`
- [ ] U3 rest — Einfach/Profi in „Ich“ (the „Audio und Sprechen“ part is done): `AppSettings` (view, theme, hotkeys,
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
- [ ] `lk load-test` with 20 publishers plus 20 presence clients against the dev server
      (needs the `lk` CLI)
- [ ] Presence: show a person as "maybe gone" when their presence link is stale instead of
      waiting for LiveKit's 20–23 s timeout for a dead client (S6)
- [ ] Tray icon and autostart (from S3): yAPPA in the notification area, start with Windows
      as an option in „Ich“
- [ ] Send-chain headroom: the pump with DeepFilterNet peaks at 3.3 ms of the 5 ms budget
      on a Ryzen 7 7700X (E1) — measure on the gang's slowest PC; if it gets close, run the
      denoiser on its own thread with a one-block handoff, or default that PC to RNNoise
- [ ] libDF on tract ≥ 0.21.16 (upstream or our patch), then remove the three advisory
      exceptions in `deny.toml` (ADR-008)
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

- [x] S3 PTT (implementation): `crates/platform` — global low-level hooks via `rdev`
      (passes keys on to the game), PTT key with auto-repeat filtered, mouse side buttons,
      left/right click never; key capture; the shell sends `SetPttPressed`; a hook that
      cannot start degrades to "only while focused" with a note in the UI. Tray and
      autostart are left for later (2026-10-23)
- [x] U3 part „Audio und Sprechen“ (pulled forward for the testabend): speaking mode
      (automatic / hold a key / always on, also as presence `talk`), threshold with the
      level after the noise suppression, PTT key with capture, noise suppression choice,
      input channel, master volume; `yappa-control channel add|list` for channels before
      U6; `docs/testabend.md` (2026-10-23)
- [x] E1 Chain in the pump (ADR-008): `audio-engine::chain` — HPF → denoiser (built on the
      pump thread) → gate (VAD) / push-to-talk / always on (linear 5 ms ramps) → EQ →
      compressor; settings reach the pump as `ChainConfig`; `Command::SetPttPressed`,
      `Event::StageLevels`, `Stats.chain_latency_ms`; the app builds with DeepFilterNet.
      DeepFilterNet's SNR is no longer fed to the gate (held it open in a silent room).
      Measured with the real devices: pump incl. chain mean 1.49 ms, max 3.3 ms per block
      (was 9.8 µs / 62 µs), chain latency 10 ms, gate closed in a silent room (2026-10-23)
- [x] S2 DSP chain offline: `crates/denoise` (`Denoiser` trait, RNNoise, DeepFilterNet 3 LL
      behind feature `deepfilter`, libDF pinned + tract 0.21.4), `dsp::Gate` (hysteresis,
      hold, fades, speech-probability veto) and `dsp::Compressor` (soft knee);
      `crates/spike-s2`: chain 41 µs (RNNoise) / 0.66 ms (DeepFilterNet) per block, both
      +10 ms, noise floor -40.5 → -73.9 dBFS (RNNoise) / silence (DFN) → DFN default,
      ADR-008, three tract advisories accepted → `docs/spikes/s2-dsp.md` (2026-10-23)
- [x] E0 Engine prep (realtime review 2026-10-23): capture ring and pump in f32, quantised
      to 16 bit only at the transport; the capture callback writes one chunk per callback;
      input channel choice (`input_channel`) and master volume (`output_gain_db`) as
      additive settings; `dsp::Limiter` at the end of the mix instead of a hard clamp;
      biquad state flushed below 1e-20 (denormals) without processor flags; pump busy time
      in `Stats` and the diagnosis; mixer bench; `AppInfo` generated from engine-protocol.
      Measured before/after (Ryzen 7 7700X, release): mixer 20 speakers 10.17 → 10.76 µs,
      EQ 5.32 → 5.35 µs, limiter 0.83 µs, pump mean 7.8 → 9.8 µs (max 46 → 62 µs, SDK
      noise). Cushion was already tied to the callback size (2026-10-23)
- [x] U5 Bind the client: `tauriApi.ts` (the door in the app: `api_*`, `voice_join`,
      `voice_leave`, `presence` event; admin answers "not yet" until U6), deep link
      `yappa://invite/…` via `tauri-plugin-deep-link` + `tauri-plugin-single-instance`
      (cold start and while running, the link lands in the login card; already logged in →
      a note), config banner only for "unreachable", config reloaded after login; tester
      guide and glossary rewritten for invite links; mock scene `invite` (2026-10-23)
- [x] U2 Voice module: channel tree TeamSpeak-style (people under their channel, counts,
      states as words: stumm/PTT/abwesend, speaking ring), join on one click or Enter or
      from the palette, switch by clicking another channel, leave from the dock; tiles of
      my channel; login card for invite links (asks for a name) that still takes developer
      invites; „Ich → Konto“ with logout; UI door reshaped to the C1 commands
      (`resume`, `redeem`, `joinChannel`, …) with `Gang` state; mock scene `login`; App
      takes its doors as props so every test picks a scene (2026-10-23)
- [x] C1 `crates/client` (ADR-007): API client with offline cache, session token in the OS
      credential store (`keyring` 3.6, file fallback), `Presence` over
      `transport::presence::PresenceLink` (snapshot at every connect + events, S6),
      `AppSettings` v1 and engine `Settings` v2 with v1 migration, profiles via
      `YAPPA_PROFILE`; invite links name their server (`InviteLink`); shell commands
      `api_*`, `voice_join`/`voice_leave` (tokens never reach the webview), `presence`
      event; headless example logs in and joins channels; `cargo xtask livekit presence`
      (hidden watcher) (2026-10-23)
- [x] S6 Control plane (ADR-005): `crates/control` (`yappa-control`: axum + sqlx/SQLite +
      livekit-api) with invites, members, roles, kick, channels, voice and presence tokens,
      `GET /config` with min version / kill switch, signed webhook → event log; bootstrap
      admin invite at first start, `invite`/`policy` subcommands; `cargo xtask control`;
      Docker image + `control` service and Caddy `/api/` route in `home.yml`;
      `crates/spike-s6` measured the presence room (attribute change ≈ 1 ms, late joiner
      sees state, kick 10 ms, dead client 20–23 s) → `docs/spikes/s6-control.md`
      (2026-10-23)
- [x] U1 Shell (ADR-004): zones A–G as components in `lib/shell/` (rail with label + icon,
      context sidebar, voice dock owned by the shell, topbar with breadcrumb, `Strg K`
      palette on Bits UI Command, banner slot, inspector); modules „Voice“ (room + people
      TeamSpeak-style, join card, tiles, person inspector) and „Ich“ (audio, appearance,
      diagnosis = former debug panel, help); banners for engine errors, reconnect, server
      unreachable, minimum version/kill switch; `Esc` closes the inspector or steps back;
      theme remembered; tester guide and glossary follow the new places (2026-10-23)
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
