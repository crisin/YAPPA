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
- [ ] Homeserver: first call from outside over the IPv6-only server (a gang member with
      IPv6, phone hotspot is enough); then ask the cable provider for a public IPv4

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

**A · Audio block** (ADR-009, interview 2026-10-23) — before M4; order is the decision


**M4 · Operations** — auto-update, admin, onboarding, banners (after the audio block, ADR-009)

- [ ] U7 System states (review 2026-10-23): offline state in the dock with „Erneut
      verbinden“; engine messages that reach the banner in German; install the downloaded
      update on the next start as well (today it waits for the click and is gone after a
      restart); „Was ist neu“ from the changelog in the banner (today only on the Updates
      page)
- [ ] Lofi round 2 (user): draw reconnect/offline/update banner and lobby radio against the
      running shell (detail questions follow established patterns, vision section 12)
- [ ] Infra for three stages (vision section 12): one compose set for LiveKit + control +
      Caddy, `home.yml` (mini PC) and new `vps.yml` differ only in `.env`/overrides; VPS is
      the update host; `docs/homeserver.md` gets a VPS part
- [ ] Optics round (user, after U3): 2–3 token variants on the running shell, then ADR

## Later

- [ ] Mixer varispeed cost (after A6): 20 speakers went from 14 to 37 µs per 10 ms block
      (0.4 % of a core — in budget, but 2.6×). Problem while gaming: none today; headroom
      for weaker machines and for plugins (phase 3). Plane: engine only, `dsp::Varispeed`
      hot loop (Hermite per sample for every speaker even at ratio 1.0 — a bypass when the
      controller sits at 1.0 ± a few ppm, or a block-wise Hermite). Not built: SIMD.
- [ ] Engine edges from the review 2026-10-23 not in the audio block: a pump panic
      reaches the UI as an error and falls back to RNNoise; automatic RNNoise fallback
      when the chain tears the 5 ms budget (ADR-009: PCs are current, nice-to-have); the
      running denoiser is reported, not the configured one; PTT hook watchdog (Windows
      drops a slow hook silently, `is_global` keeps saying true) and a missed key-up;
      engine settings saved on quit, not only on the stats tick; `validate` checks
      EQ/compressor for NaN; the engine failing to start shows a window with the error
      instead of dying
- [ ] Control-plane edges from the review 2026-10-23: cache the presence token and the
      channel tokens (≈ 12 h, vision section 3) so a start without the control plane still
      has presence; kick revokes the session and a member can be removed; healthcheck in
      the compose files; voice grant limited to the microphone source; a Linux CI job that
      builds `crates/control` (+ the Docker image); actions pinned to SHAs
- [ ] UI edges from the review 2026-10-23: focus follows a module switch and the inspector;
      channel buttons stay focusable during a join; per-person volume and mute survive a
      restart; mute state after a webview reload; tests for the `reconnect`/`offline`/
      `update` scenes, engine-error banners and a failed join
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
- [ ] Control-plane image from CI (`ghcr.io/crisin/yappa-control`), so the box pulls instead
      of compiling Rust (needs 4 GB RAM today); `home.yml` keeps `build:` as the fallback
- [ ] Send-chain headroom: the pump with DeepFilterNet peaks at 3.3 ms of the 5 ms budget
      on a Ryzen 7 7700X (E1) — measure on the gang's slowest PC; if it gets close, run the
      denoiser on its own thread with a one-block handoff, or default that PC to RNNoise
- [ ] libDF on tract ≥ 0.21.16 (upstream or our patch), then remove the three advisory
      exceptions in `deny.toml` (ADR-008)
- [ ] Bumps when the files are touched anyway: LiveKit server v1.13.10, `livekit` 0.9.6
- [ ] Phase 2 · Quality: second media server with failover (presence room needs a home,
      ADR-005 revisit), Prometheus/Grafana, load, AEC, four weeks without an outage
- [ ] Soundboard round 3 (ADR-010): hotkeys per sound (same hook as PTT, a key table in
      the app settings), library sync between clients (byte streams on the presence room,
      hash-addressed), drift compensation on the local slot if an evening shows gaps.
      *Problem while gaming:* a meme needs a key, not a mouse trip. *Planes:* client +
      presence room for the sync; no control plane. *Not built:* server storage, a shared
      library owned by the admin.
- [ ] App audio (user, 2026-10-26, next after the soundboard): stream a game's or player's
      sound into the channel — `AppCapture` behind a trait (WASAPI process loopback on
      Windows, macOS gets what comes along), the `music` track of the planning doc, a stage
      tile. *Problem while gaming:* sharing a clip or a stream means a second app today.
      *Planes:* engine (capture + own track) and transport (third track kind, already in
      `TrackKind`); no control plane. *Not built:* video, system-wide loopback, per-app
      volume for the listeners beyond the per-track one.
- [ ] Phase 3 · Extensions: lobby radio, effects, S4/CLAP, text chat (`chat` topic), hub
      module
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

- [x] Soundboard round 2 (2026-10-26): per-sound volume (−30..=12 dB, `Sound.gain_db` from
      a `soundboard.json` next to the files, `PlaySound.gain_db` and `SetSoundGain` for a
      change while the clip runs — `Voice.gain` in the sound thread), „Löschen“ to the
      recycle bin (`trash` crate, folder-only paths), the engine re-decodes a file replaced
      under the same name (size + mtime stamp in the cache), and the other side flags a
      playing soundboard track: `Mixer::sound_levels` → `PeerLevel.sound` with the
      `speaking` hold, shown on the stage card, in the person inspector and as a note in the
      channel list (mine from `Event::Sounds`). Protocol additive, no version bump. Tests:
      sound thread (own gain live, replaced file), shell sidecar (write/forget/refuse
      outside the folder/damaged file), protocol shapes, UI (edit mode live + remembered,
      delete, „spielt einen Sound“ on card, sidebar and inspector).
- [x] Tray icon and autostart (2026-10-25): the cat sits in the notification area (Tauri
      `tray-icon`); left click shows the window, the menu has „Fenster zeigen“, „Stumm“ and
      „Taub“ (check marks follow the engine's `MuteState`) and „Beenden“ — the only thing
      that ends the app, through the same `RunEvent::Exit` path that drops engine and
      presence. App setting `on_close: tray | quit` (default tray, so a reflex click on X
      never ends a call; `WindowEvent::CloseRequested` is prevented and the window hidden)
      and `autostart: bool` (`tauri-plugin-autostart`, Run key / launch agent, started with
      `--minimized` so the app stays in the tray); both under „Ich → Darstellung“. App
      settings file version 2 with a migration test (a version-1 file loads with the
      defaults). Without a tray (OS refused) the close goes through as before. Glossary
      „Infobereich“, tester guide and „So funktioniert es“ updated.
- [x] Soundboard POC (ADR-010, 2026-10-25): a `yappa-sounds` thread in the engine plays
      files (symphonia: WAV, FLAC, OGG, MP3, M4A → 48 kHz mono via the A1 resampler, cut
      at 60 s, cached) one 10 ms block at a time, self-paced a block ahead, up to eight
      overlapping through the soundboard gain and a limiter — out on a second LiveKit
      track `soundboard` (96 kbit/s mono, DTX, published for the whole session next to
      the voice) and into a second local mixer slot of the monitor's kind, so the clicker
      hears it. Receivers key mixer slots by (identity, track): `SetPeerVolume { track:
      soundboard }` and the mute land on that track, no leveler on it, `SLOTS` 24 → 48.
      Protocol additive: `PlaySound`, `StopSounds`, `Event::Sounds`,
      `Settings.soundboard_gain_db`. The library is the folder `<app data>/soundboard/`
      (shell: list, add a picked file over IPC, open in Explorer). UI: inspector
      „Soundboard“ (search, grid with the playing state, Stopp, my volume, add, open
      folder) from a button next to the room title and the palette; the person inspector
      gets a soundboard volume and mute. Glossary and help. Tests: decoder (stereo 44.1 →
      mono 48, cut, garbage), the thread (overlap, limiter, stop, failure), mixer (second
      track next to the voice, local slot), engine integration (out on the track, into
      the own mix, error for a missing file, per-track mute), shell (names, listing), UI
      (list, play, stop, search, gain, add, refuse, per-person). Not verified: a real
      call over the server with two clients (the user's review step), bandwidth of the
      idle track, pacing on a PC without a raised timer resolution
- [x] D1 Release pipeline (ADR-006, 2026-10-25): `cargo xtask release <channel>` —
      version bump (`Cargo.toml` as the one source, npm files follow), changelog cut
      Unreleased → `## <version> (<date>)` with the notes as release notes, signed
      `cargo xtask build` (`createUpdaterArtifacts`, minisign key only in `.env`,
      `release keygen` makes it outside the repo), `latest.json` in the updater's static
      format merged with the other OS's entry of the same version, upload to GitHub
      Releases by default (`v<version>` + rolling `<channel>`; user decision 2026-10-25,
      ADR-006 amendment — the box is IPv6-only) or to a folder (Caddy `/updates/` from
      `infra/livekit/updates/`, WSL path from the dev PC) or scp; git stays the user's. Shell:
      `tauri-plugin-updater` + `tauri-plugin-process`, endpoint
      `<update_base_url>/<channel>/latest.json` per check, download + signature check at
      once, install on click (`install_update`, leaves the engine first, restart). UI:
      hourly config + check, „Update bereit“ banner (dismissible, leaves the call before
      installing), blocked/killed banner forces the leave and offers the install, Updates
      page with check button, status and „Was ist neu“; scenes `update` / `update-ready`.
      Unit tests for bump, changelog cut, manifest merge (snapshot), targets, `.env`; UI
      tests for the banners and the page. Not yet run end to end against a real host (the
      first `release stable` is the user's, after the review); macOS artifacts untested
- [x] A5 Monitor „Mich selbst hören“ (ADR-009, 2026-10-25): a slot kind of its own in the
      mixer — the pump pushes the send chain's f32 output (after the limiter, before the
      mute) through a `MonitorFeed` into an `rtrb` ring of 200 ms, the output callback adds
      it to the mix without leveler or per-person volume, with the same cushion rule as the
      speakers (one block more than the callback asks for) and a trim back to the cushion
      when the clocks drift apart (cushion capped at half the ring, so a 2048-frame callback
      still works — realtime review); no flush flag on the way in (the pump's first block would
      race it), off drains the ring on the next callback. `Command::SetMonitor`,
      `Event::MonitorState`, `Stats.monitor` (latency estimated from capture buffer + chain
      delay + ring depth + playout buffer, plus underruns and dropped blocks; also in the stats
      log line). The
      Signalkette page switches it on while open and off on leave; `AppSettings.listening`
      (headset/speakers, from the onboarding) — on speakers the page warns of feedback and
      does not start it by itself. Glossary entry, diagnosis row, mock engine, UI tests.
      Measured (Ryzen 7 7700X, release): mixer 20 speakers 13.53 (A4 baseline; a run before this change
      under a parallel build gave 18.3) → 13.5–13.9 µs per 10 ms block with the monitor off and
      13.8–13.9 µs with it on (two runs; the difference is within the noise).
- [x] A6 Drift compensation (ADR-009, 2026-10-25, closes the ADR-003 known gap): per mixer
      slot a `dsp::DriftController` (fill level at callback start, averaged over 1.5 s; PI
      with 3 s horizon, `k_i = k_p²/4`, anti-windup; ratio within 1 ± 0.5 %, the learned
      drift capped at ±2000 ppm) drives a `dsp::Varispeed` (4-point Hermite, variable
      ratio, one input per output from the first sample — no priming wait, no kernel table)
      that pulls the speaker's ring in the callback; target = cushion + half a frame. The
      ≈ 80 ms skip stays as the emergency brake above two cushions and resets the controller.
      Diagnosis: `SpeakerStats.drift_ratio` and `playout_fill_ms` (additive, serde
      defaults), „Drift-Ausgleich“ in the diagnosis and the glossary, `drift=` in the
      per-second stats log line; `playout_skips` stays. Tests: proptest ±200 ppm × five
      callback sizes over five simulated minutes (never skipped, never dry, settles at the
      cushion), mixer tests with a fake drifting clock over ten minutes (±200 ppm, 480; ±150
      ppm at 1056 and 160 frames per callback: 0 skips, 0 underruns, ratio carries the
      drift's sign). Measured (Ryzen 7 7700X, release, back-to-back): mixer 20 speakers
      14.18 → 37.34 µs per 10 ms block (≈ 1.2 µs per speaker, 0.4 % of the block); bench table
      in `docs/spikes/s2-dsp.md`. Not verified: a real long call (the first evening's
      diagnostics file should show `playout_skips` 0 and `drift` a few hundred ppm).
- [x] A4 Loudness levelling at the receiver (ADR-009, 2026-10-25): `dsp::Leveler` per
      mixer slot — speech-gated RMS over 400 ms (blocks under −50 dBFS neither measured nor
      acted on, so pauses are not pumped up), target −20 dBFS, ±12 dB, +3 / −6 dB per
      second, measured before the gain (open loop), gain of the previous block applied in
      the same pass over the ring chunk; the manual per-person volume stays an offset on
      top; off (Profi → „Audio und Sprechen“) slides every gain back to 0 dB. `dsp::Limiter`
      as the 7th stage of the send chain (−1 dBFS, 80 ms), against the 20 "Possible
      clipping detected" of 2026-10-24 (compressor makeup +3 dB on a hot DualSense mic).
      Diagnosis: `Stats.speakers` with the leveler gain per speaker, also in the per-second
      stats log line. Measured before/after (Ryzen 7 7700X, release): mixer 20 speakers
      11.42 → 13.53 µs per 10 ms block (0.1 µs per speaker), leveler alone 0.71 µs,
      limiter 1.09 µs — the send chain gains ≈ 1 µs of its 5 ms budget.
- [x] A3 Global mute and deafen keys (ADR-009, 2026-10-25): „Stumm“ and „Taub“ next to
      push-to-talk in Profi → „Tasten“, same hook (`platform::Hotkeys` now carries three
      actions), same capture dialog, one key does one thing (`AppSettings::assign_key`
      names the holder). The engine owns the state (`audio_engine::mute::MuteState`:
      deafen implies mute, undeafen gives the microphone back as it was) and answers every
      change with `Event::MuteState`; key, dock button and UI all go through
      `ToggleMuted`/`ToggleDeafened`. Presence carries `muted`/`deaf` (ADR-005), so the
      channel tree shows „stumm“/„taub“ behind the name for everybody. Keys survive a
      restart (app settings). Verified: unit tests in mute/platform/client/api-types,
      engine integration test (silence on the wire while deafened), UI tests, browser mode
      by hand; not verified: a real key press in a fullscreen game (user, testabend).
- [x] Audio quality tiers (2026-10-25): profiles Sparsam/Standard/Hoch/Studio/Stress in
      the settings schema (v3, migration from the bare bitrate), the values behind them
      (bitrate, mono/stereo, RED, DTX) editable in Profi → „Netzwerk“, a profile change
      republishes the voice track inside the running call, the diagnosis shows profile,
      negotiated bitrate and payload rate; measured per profile (libwebrtc caps Opus at
      ≈ 256 kbit/s, RED stops fitting above ≈ 200) in
      `docs/spikes/quality-tiers.md`. Frame length, FEC and complexity are fixed in
      libwebrtc and shown as facts.
- [x] A2 Device return (ADR-009, 2026-10-25): `platform::DeviceWatcher` — Windows
      `IMMNotificationClient` on the MMDevice enumerator, bursts coalesced on a thread of
      its own (300 ms quiet), the shell turns a change into `RefreshDevices`; macOS has no
      watcher yet (`start` gives `None`) and degrades to the engine's polling. Engine: a
      failed stream is reopened on the chosen device first, then the default; while a side
      runs on a stand-in the device list is polled once a second and the chosen device
      taken back as soon as it is listed; with „Standardgerät“ chosen a new system default
      is followed on refresh. New `Event::DeviceState` (`DeviceSlot` per side: open name,
      fallback, missing name) replaces the error banner for a missing device: the UI shows
      a banner and a line in the microphone/output settings that go by themselves when the
      device is back. Fakes cover unplug, return, refresh and a moved default; the real
      pull-and-plug of a USB headset in a call is the user's review step.
- [x] Unreachable reason (first evening 2026-10-24): `client::api` classifies a missing
      answer (connect/timeout/TLS/other), resolves the host once more to tell "no such
      name" from "IPv6-only server" from "server down", logs the error chain at warn;
      the login card, the offline card and the banner show the reason; tester guide has
      the table.
- [x] Homeserver walkthrough (user, hands-on, 2026-10-24): on the dev PC instead of the mini
      PC, Ubuntu in WSL2 (mirrored network) instead of a VM. FRITZ!Box 6660 Cable sits
      behind CGNAT (box IPv4 `100.96.…`, no DS-Lite flag), so the server runs IPv6-only
      with `livekit.home-ipv6.yaml`; Let's Encrypt reached port 443 over IPv6 and issued
      the certificate, the admin invite came up. Guide amended: reliable CGNAT check, IPv6-only section, WSL2 variant,
      FRITZ!Box traps.
- [x] A1 Resampling (ADR-009): `dsp::Resampler` — windowed-sinc polyphase, 128 phases,
      32 taps (×ratio when going down), pull API without allocation; capture ring carries
      the device rate and the pump converts, playout converts in the callback
      (`audio-engine::resample::ResampledRender`, one render per callback with the exact
      demand); `pick_mode` takes 48 kHz, else the device's default rate, else its highest;
      48 kHz path unchanged. Cost 12.3–12.5 µs per 10 ms block for 16→48, 48→16 and
      44.1→48 kHz (0.25 % of the budget); latency half a kernel per side (1.0 ms at 16 kHz,
      0.4 ms at 44.1 kHz) plus at most two samples; realtime review's four findings
      applied. „Audio und Sprechen“ says when the mic runs at 16 kHz (Bluetooth hands-free,
      phone quality) or 44.1 kHz. Fake 16 kHz mic in the engine tests; not yet tried on a
      real Bluetooth headset (2026-10-24)
- [x] ADR-009 Audio first: interview with the user (eleven questions: hardware, default
      speaking mode, lost device, weakest PC, network, loudness, PTT mechanics, mute keys,
      monitor, D1 order) → audio block A1–A6 on the board before M4; D1/U7 wait;
      installers by hand until then (2026-10-23)
- [x] Review 2026-10-23 (`docs/02-review-2026-10-23.md`): three reviewers over engine,
      server/client and desktop; verdict „on track, code ahead of testing“; nine small
      fixes before the first evening (ghost join — presence set before the join and
      cleared on Disconnected, offline-at-start card, join errors in the banner, engine
      errors kept per source and handed over on resync, Esc in key capture/tooltips, 502
      as unreachable for the cache, denoiser warning throttled, 48 kHz pre-check in
      `mittesten.md`, the flaky `against_control` test — pid reuse hit a stale test DB);
      the rest on the board as U7 and three „edges“ lines (2026-10-23)
- [x] Cloudflare Tunnel setup: `infra/livekit/tunnel.yml` (LiveKit + control plane, no
      Caddy; optional `cloudflared` profile), two hostnames (voice → 7880, API → 8090,
      `YAPPA_PUBLIC_URL` set), `docs/cloudflare-tunnel.md` — says plainly that the voice
      (UDP 7882 / TCP 7881) cannot go through the tunnel and needs port forwarding and a
      public IPv4; compose validated, never run with a real tunnel (2026-10-23)
- [x] U6 Admin module (rail bottom, admins only — `shell.setRole` now follows the login):
      Leute (role, where they are from presence, app version marked when below the policy,
      last seen, kick after a second click), Einladungen (new link to copy, list with
      open/used/expired, withdraw), Channels (add, rename, move, delete; the channel tree
      follows), Betrieb (minimum version and kill list with a lock-myself-out warning, event
      log); Tauri commands over the existing `crates/client` admin calls, `inviteLink` and
      `setPolicy` added to the API door (2026-10-23)
- [x] U4 Onboarding: full window before the shell while nobody is logged in — 1 Name (the
      login card), 2 Mikro (device, input, live level, output), 3 Reden (headset or
      speakers; speakers preselect „Taste halten“ because v1 has no echo cancelling, then
      the speaking mode with threshold or key) → joins the first channel (Lobby). Steps 2
      and 3 are the parts `MicTest` / `TalkMode` that „Audio und Sprechen“ is now built
      from; a developer invite still goes straight into its room (2026-10-23)
- [x] U3 rest — Einfach/Profi in „Ich“: the switch on top of the settings navigation, stored
      as `AppSettings.view`; Einfach = Audio und Sprechen, Darstellung, Benachrichtigungen,
      Konto, Hilfe; Profi adds Signalkette (six stages with live level and state, cost per
      10 ms block and added latency on top, parameters for high-pass, denoiser, gate, EQ
      bands, compressor), Tasten, Netzwerk (bitrate), Updates (channel, stored until D1),
      Diagnose; theme now lives in `AppSettings` (browser storage only as first-frame
      cache); palette command for the view (2026-10-23)
- [x] Server setup made simple: `cargo xtask local [--lan]` (LiveKit + control plane in one
      command; LAN mode uses this PC's address for links and media — tested end to end with
      a join over 192.168.178.60), `cargo xtask control --lan …`; `home.yml` fixes the key
      name `yappa` (`.env` = secret + domain only); `docs/homeserver.md` rewritten as one
      path (clone, two-line `.env`, one compose command, admin link, invites, channels,
      backups, troubleshooting) (2026-10-23)
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
